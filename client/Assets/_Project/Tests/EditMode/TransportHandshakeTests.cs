// U-5a: does Unity's Mono ClientWebSocket actually put Authorization on the upgrade request?
//
// The API exists on the netstandard2.1 surface and in Unity's Mono System.dll (checked in the
// p0-02 client review), but "the method compiles" and "the header reaches the wire" are
// different claims - some Mono builds treat Authorization as a restricted header and throw.
// If it does not reach the wire, every AC that depends on authentication is unreachable and
// ADR-0005 section 6's ticket path has to be pulled forward, so this is worth a real socket.
//
// No server and no HttpListener: a raw TcpListener on a loopback port accepts the connection,
// reads the request head and hangs up. ClientWebSocket then fails the upgrade, which is fine -
// by that point the bytes we care about have been read. HttpListener would have needed a URL
// reservation or admin rights on Windows.

using System;
using System.Net;
using System.Net.Sockets;
using System.Security.Cryptography;
using System.Text;
using System.Threading;
using System.Threading.Tasks;
using NUnit.Framework;
using Starfall.Net;

namespace Starfall.Tests.EditMode
{
    [TestFixture]
    public sealed class TransportHandshakeTests
    {
        const int HandshakeTimeoutMs = 5000;

        [Test]
        public void Upgrade_CarriesTheAuthorizationHeader()
        {
            string token = DevAuthToken.Create(DevAuthToken.DefaultSubject, "dev_only_not_a_secret");

            var listener = new TcpListener(IPAddress.Loopback, 0);
            listener.Start();
            int port = ((IPEndPoint)listener.LocalEndpoint).Port;

            Task<string> requestHead = ReadFirstRequestHead(listener);

            var log = new RecordingLogSink();
            using (var transport = new PcWebSocketTransport(log))
            {
                transport.Connect("ws://127.0.0.1:" + port + "/ws", token);

                Assert.That(requestHead.Wait(HandshakeTimeoutMs), Is.True,
                    "no HTTP request arrived within " + HandshakeTimeoutMs + " ms");

                string head = requestHead.Result;
                TestContext.WriteLine("--- upgrade request as it reached the socket ---");
                TestContext.WriteLine(head);

                Assert.That(head, Does.StartWith("GET /ws HTTP/1.1"));
                Assert.That(head.IndexOf("Authorization: Bearer " + token, StringComparison.OrdinalIgnoreCase),
                    Is.GreaterThanOrEqualTo(0),
                    "Mono did not put the Authorization header on the upgrade request. " +
                    "If this fails, tell architect: ADR-0005 section 6's ticket path has to come early.");

                // The rest of the handshake the server relies on.
                Assert.That(head, Does.Contain("Upgrade: websocket").IgnoreCase);
                Assert.That(head, Does.Contain("Sec-WebSocket-Key").IgnoreCase);
                Assert.That(head, Does.Contain("Sec-WebSocket-Version: 13").IgnoreCase);

                foreach (string line in log.Lines) TestContext.WriteLine(line);
            }

            listener.Stop();
        }

        [Test]
        public void FailedUpgrade_ReportsDisconnectWithoutAStatusCode()
        {
            // The other half of U-5c, as behaviour rather than prose: a refused upgrade gives
            // the client an exception with no HTTP status attached, which is why the reconnect
            // policy cannot single out 401 and why authentication evidence is always
            // server-side (ADR-0005 sections 5 and 6).
            var listener = new TcpListener(IPAddress.Loopback, 0);
            listener.Start();
            int port = ((IPEndPoint)listener.LocalEndpoint).Port;
            Task<string> requestHead = ReadFirstRequestHead(listener, respondWith401: true);

            var log = new RecordingLogSink();
            DisconnectInfo? seen = null;

            using (var transport = new PcWebSocketTransport(log))
            {
                transport.Disconnected += info => seen = info;
                transport.Connect("ws://127.0.0.1:" + port + "/ws", "token");

                Assert.That(requestHead.Wait(HandshakeTimeoutMs), Is.True, "no request arrived");

                // The failure is reported through the pump, like everything else.
                var deadline = DateTime.UtcNow.AddMilliseconds(HandshakeTimeoutMs);
                while (!seen.HasValue && DateTime.UtcNow < deadline)
                {
                    transport.Pump();
                    Thread.Sleep(10);
                }
            }

            listener.Stop();

            Assert.That(seen.HasValue, Is.True, "a refused upgrade must still produce a Disconnected notification");
            TestContext.WriteLine("disconnect reported as: " + seen.Value);

            Assert.That(seen.Value.Kind, Is.EqualTo(DisconnectKind.Failed));
            Assert.That(seen.Value.CloseCode, Is.Null,
                "a refused upgrade has no WebSocket close code - and no readable HTTP status either");
        }

        // ------------------------------------------------------------------ close-code readability (R3 1.3, decision 5)
        //
        // The client-must-not-reconnect-on-4001 rule (01_architect_decisions.md "R3 추가
        // 판정" section 1.3) only makes sense if a real ClientWebSocket connection actually
        // exposes an application-range close code like 4001 through CloseStatus. That is a
        // different claim from "401/503 look the same" (U-5c, tested above) - this is a Close
        // frame on an ESTABLISHED socket, which System.Net.WebSockets does expose
        // (PcWebSocketTransport.cs ReceiveLoop reads result.CloseStatus into DisconnectInfo).
        // Architect's instruction was explicit: confirm this actually reads before building
        // policy on top of it, and stop if it does not. This test performs a real WebSocket
        // handshake over a raw loopback socket (no HttpListener - same reasoning as
        // ReadFirstRequestHead's header comment: no URL reservation needed) and has the "server"
        // send a genuine Close frame with code 4001, unmasked, exactly as RFC 6455 requires for
        // a server->client frame.
        [Test]
        public void RemoteClose_WithApplicationRangeCode4001_IsReadableAsCloseCode()
        {
            var listener = new TcpListener(IPAddress.Loopback, 0);
            listener.Start();
            int port = ((IPEndPoint)listener.LocalEndpoint).Port;

            Task serverSide = AcceptUpgradeThenSendClose(listener, closeCode: 4001, reason: "superseded by newer session");

            var log = new RecordingLogSink();
            DisconnectInfo? seen = null;

            using (var transport = new PcWebSocketTransport(log))
            {
                transport.Disconnected += info => seen = info;
                transport.Connect("ws://127.0.0.1:" + port + "/ws", null);

                var deadline = DateTime.UtcNow.AddMilliseconds(HandshakeTimeoutMs);
                while (!seen.HasValue && DateTime.UtcNow < deadline)
                {
                    transport.Pump();
                    Thread.Sleep(10);
                }
            }

            listener.Stop();
            Assert.That(serverSide.Wait(HandshakeTimeoutMs), Is.True, "the fake server task did not complete");

            foreach (string line in log.Lines) TestContext.WriteLine(line);
            Assert.That(seen.HasValue, Is.True, "a server-initiated close must still produce a Disconnected notification");
            TestContext.WriteLine("disconnect reported as: " + seen.Value);

            Assert.That(seen.Value.Kind, Is.EqualTo(DisconnectKind.Remote));
            Assert.That(seen.Value.CloseCode, Is.EqualTo(4001),
                "4001 has no named WebSocketCloseStatus member, but the raw close code must " +
                "still survive to DisconnectInfo.CloseCode - this is the load-bearing premise " +
                "behind 'the client does not auto-reconnect on 4001' (R3 decision 5). If this " +
                "assertion fails, that policy's whole premise is false and architect must be told.");
        }

        /// <summary>Accepts one connection, completes a real WebSocket upgrade (computes
        /// Sec-WebSocket-Accept per RFC 6455 section 1.3), then sends one unmasked Close frame
        /// carrying <paramref name="closeCode"/> and closes the TCP connection.</summary>
        static Task AcceptUpgradeThenSendClose(TcpListener listener, int closeCode, string reason)
        {
            return Task.Run(async () =>
            {
                using (TcpClient client = await listener.AcceptTcpClientAsync().ConfigureAwait(false))
                using (NetworkStream stream = client.GetStream())
                {
                    var head = new StringBuilder();
                    var buffer = new byte[1024];
                    while (head.ToString().IndexOf("\r\n\r\n", StringComparison.Ordinal) < 0)
                    {
                        int read = await stream.ReadAsync(buffer, 0, buffer.Length).ConfigureAwait(false);
                        if (read <= 0) break;
                        head.Append(Encoding.ASCII.GetString(buffer, 0, read));
                    }

                    string key = ExtractHeaderValue(head.ToString(), "Sec-WebSocket-Key");
                    Assert.That(key, Is.Not.Null, "test bug: no Sec-WebSocket-Key on the upgrade request");

                    byte[] response = Encoding.ASCII.GetBytes(
                        "HTTP/1.1 101 Switching Protocols\r\n" +
                        "Upgrade: websocket\r\n" +
                        "Connection: Upgrade\r\n" +
                        "Sec-WebSocket-Accept: " + ComputeAcceptKey(key) + "\r\n\r\n");
                    await stream.WriteAsync(response, 0, response.Length).ConfigureAwait(false);

                    byte[] closeFrame = BuildCloseFrame(closeCode, reason);
                    await stream.WriteAsync(closeFrame, 0, closeFrame.Length).ConfigureAwait(false);
                    await stream.FlushAsync().ConfigureAwait(false);

                    // Give ClientWebSocket time to read and process the close frame before the
                    // TCP connection under it goes away.
                    await Task.Delay(300).ConfigureAwait(false);
                }
            });
        }

        /// <summary>RFC 6455 section 5.5.1: a single, unmasked Close frame (server frames are
        /// never masked) whose payload is the 2-byte big-endian close code followed by the
        /// optional UTF-8 reason. No payload here exceeds 125 bytes, so the short length form is
        /// always enough.</summary>
        static byte[] BuildCloseFrame(int closeCode, string reason)
        {
            byte[] reasonBytes = string.IsNullOrEmpty(reason) ? Array.Empty<byte>() : Encoding.UTF8.GetBytes(reason);
            byte[] payload = new byte[2 + reasonBytes.Length];
            payload[0] = (byte)(closeCode >> 8);
            payload[1] = (byte)(closeCode & 0xFF);
            Array.Copy(reasonBytes, 0, payload, 2, reasonBytes.Length);

            Assert.That(payload.Length, Is.LessThan(126), "test bug: close payload too long for the short length form");

            var frame = new byte[2 + payload.Length];
            frame[0] = 0x88; // FIN=1, opcode=8 (Close)
            frame[1] = (byte)payload.Length; // MASK bit clear: server->client frames are unmasked
            Array.Copy(payload, 0, frame, 2, payload.Length);
            return frame;
        }

        /// <summary>RFC 6455 section 1.3's handshake: base64(SHA1(key + magic GUID)).</summary>
        static string ComputeAcceptKey(string key)
        {
            const string magic = "258EAFA5-E914-47DA-95CA-C5AB0DC85B11";
            using (SHA1 sha1 = SHA1.Create())
            {
                byte[] hash = sha1.ComputeHash(Encoding.ASCII.GetBytes(key + magic));
                return Convert.ToBase64String(hash);
            }
        }

        static string ExtractHeaderValue(string head, string name)
        {
            foreach (string line in head.Split(new[] { "\r\n" }, StringSplitOptions.None))
            {
                int colon = line.IndexOf(':');
                if (colon <= 0) continue;
                if (string.Equals(line.Substring(0, colon).Trim(), name, StringComparison.OrdinalIgnoreCase))
                    return line.Substring(colon + 1).Trim();
            }
            return null;
        }

        /// <summary>Accepts one connection, reads up to the blank line that ends the request
        /// head, optionally answers 401, then closes.</summary>
        static Task<string> ReadFirstRequestHead(TcpListener listener, bool respondWith401 = false)
        {
            return Task.Run(async () =>
            {
                using (TcpClient client = await listener.AcceptTcpClientAsync().ConfigureAwait(false))
                using (NetworkStream stream = client.GetStream())
                {
                    var head = new StringBuilder();
                    var buffer = new byte[1024];

                    while (head.ToString().IndexOf("\r\n\r\n", StringComparison.Ordinal) < 0)
                    {
                        int read = await stream.ReadAsync(buffer, 0, buffer.Length).ConfigureAwait(false);
                        if (read <= 0) break;
                        head.Append(Encoding.ASCII.GetString(buffer, 0, read));
                    }

                    if (respondWith401)
                    {
                        byte[] response = Encoding.ASCII.GetBytes(
                            "HTTP/1.1 401 Unauthorized\r\nContent-Length: 0\r\nConnection: close\r\n\r\n");
                        await stream.WriteAsync(response, 0, response.Length).ConfigureAwait(false);
                    }

                    return head.ToString();
                }
            });
        }
    }
}
