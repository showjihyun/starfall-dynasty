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
