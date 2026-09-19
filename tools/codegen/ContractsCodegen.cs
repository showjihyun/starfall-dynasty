// STARFALL DYNASTY — contracts -> C# DTO generator.
//
// .NET 10 SDK file-based app. No NuGet dependencies: System.Text.Json only.
// Rules: docs/adr/0002-contract-format-and-codegen.md section 4.
//
//   dotnet run tools/codegen/ContractsCodegen.cs -- --contracts contracts --out <dir>
//   dotnet run tools/codegen/ContractsCodegen.cs -- --contracts contracts --out <dir> --check
//
// Design notes that are load-bearing:
//   * Unknown JSON Schema keywords are a hard error with a JSON Pointer path. Silently
//     ignoring a keyword is how quicktype dropped every allOf envelope field (ADR-0002).
//   * Integer C# types come from the declared [minimum, maximum], never from `format`.
//   * A required-and-nullable field maps to Required.AllowNull. Required.Always on such a
//     field rejects a VALID fixture (null-client-time-max-seq.json).
//   * Output is deterministic: ordinal-sorted members and files, LF endings, no BOM,
//     no timestamps and no tool version.
//   * --check compares file CONTENT and the file SET. Only *.cs participates; *.meta is
//     owned by Unity and is never created, deleted or counted here.

#nullable disable

using System;
using System.Collections.Generic;
using System.Globalization;
using System.IO;
using System.Linq;
using System.Text;
using System.Text.Json;

try
{
    return Cli.Run(args);
}
catch (CodegenException ex)
{
    Console.Error.WriteLine("codegen: " + ex.Message);
    return 2;
}

// ---------------------------------------------------------------------------

sealed class CodegenException : Exception
{
    public CodegenException(string message) : base(message) { }
}

static class Cli
{
    public static int Run(string[] args)
    {
        string contracts = null, outDir = null;
        bool check = false;

        for (int i = 0; i < args.Length; i++)
        {
            switch (args[i])
            {
                case "--contracts":
                    contracts = Next(args, ref i);
                    break;
                case "--out":
                    outDir = Next(args, ref i);
                    break;
                case "--check":
                    check = true;
                    break;
                case "-h":
                case "--help":
                    Console.WriteLine("usage: ContractsCodegen --contracts <dir> --out <dir> [--check]");
                    return 0;
                default:
                    throw new CodegenException($"unknown argument '{args[i]}'");
            }
        }

        if (contracts is null) throw new CodegenException("--contracts is required");
        if (outDir is null) throw new CodegenException("--out is required");

        string contractsFull = Path.GetFullPath(contracts);
        if (!Directory.Exists(contractsFull))
            throw new CodegenException($"contracts directory not found: {contractsFull}");

        var files = new Generator(contractsFull).Generate();
        string outFull = Path.GetFullPath(outDir);

        return check ? Check(files, outFull) : Write(files, outFull);
    }

    static string Next(string[] args, ref int i)
    {
        if (i + 1 >= args.Length) throw new CodegenException($"{args[i]} needs a value");
        return args[++i];
    }

    static int Write(SortedDictionary<string, string> files, string outDir)
    {
        Directory.CreateDirectory(outDir);
        var encoding = new UTF8Encoding(encoderShouldEmitUTF8Identifier: false);

        foreach (var kv in files)
        {
            string path = Path.Combine(outDir, kv.Key);
            string existing = File.Exists(path) ? File.ReadAllText(path) : null;
            if (!string.Equals(existing, kv.Value, StringComparison.Ordinal))
            {
                File.WriteAllText(path, kv.Value, encoding);
                Console.WriteLine($"wrote {kv.Key}");
            }
            else
            {
                Console.WriteLine($"unchanged {kv.Key}");
            }
        }

        // Generated/ belongs to the generator. A .cs left behind by a renamed type still
        // compiles, so nobody would notice it. Remove it; leave its .meta to Unity.
        foreach (string stale in ExistingCsFiles(outDir).Where(f => !files.ContainsKey(f)))
        {
            File.Delete(Path.Combine(outDir, stale));
            Console.WriteLine($"removed stale {stale}");
        }

        Console.WriteLine($"{files.Count} file(s) in {outDir}");
        return 0;
    }

    static int Check(SortedDictionary<string, string> files, string outDir)
    {
        var problems = new List<string>();

        if (!Directory.Exists(outDir))
        {
            Console.Error.WriteLine($"--check: output directory does not exist: {outDir}");
            return 1;
        }

        foreach (var kv in files)
        {
            string path = Path.Combine(outDir, kv.Key);
            if (!File.Exists(path))
                problems.Add($"missing   {kv.Key}   ({Rel(path)})");
            else if (!string.Equals(File.ReadAllText(path), kv.Value, StringComparison.Ordinal))
                problems.Add($"differs   {kv.Key}   ({Rel(path)})");
        }

        foreach (string extra in ExistingCsFiles(outDir).Where(f => !files.ContainsKey(f)))
            problems.Add($"orphan    {extra}   ({Rel(Path.Combine(outDir, extra))})");

        if (problems.Count == 0)
        {
            Console.WriteLine($"--check: up to date ({files.Count} file(s))");
            return 0;
        }

        Console.Error.WriteLine($"--check: {problems.Count} problem(s) in {outDir}");
        foreach (string p in problems.OrderBy(p => p, StringComparer.Ordinal))
            Console.Error.WriteLine("  " + p);
        Console.Error.WriteLine("Generated/ is generator output. Re-run without --check instead of editing by hand.");
        return 1;
    }

    // *.cs only. *.meta is Unity's; counting it would make every checked-out copy look dirty.
    static IEnumerable<string> ExistingCsFiles(string outDir) =>
        Directory.Exists(outDir)
            ? Directory.GetFiles(outDir, "*.cs", SearchOption.AllDirectories)
                .Select(f => Path.GetRelativePath(outDir, f).Replace('\\', '/'))
                .OrderBy(f => f, StringComparer.Ordinal)
            : Enumerable.Empty<string>();

    static string Rel(string path)
    {
        string rel = Path.GetRelativePath(Directory.GetCurrentDirectory(), path).Replace('\\', '/');
        return rel.StartsWith("..", StringComparison.Ordinal) ? path.Replace('\\', '/') : rel;
    }
}

// ---------------------------------------------------------------------------
// JSON Schema subset reader
// ---------------------------------------------------------------------------

/// <summary>A schema keyword together with the file it came from, so nested $refs resolve
/// against the right base directory even after merging across files.</summary>
readonly struct Kw
{
    public readonly JsonElement Value;
    public readonly string File;
    public Kw(JsonElement value, string file) { Value = value; File = file; }
}

static class Keywords
{
    // Interpreted: these change the generated code.
    public static readonly HashSet<string> Structural = new(StringComparer.Ordinal)
    {
        "type", "properties", "required", "$ref", "$defs", "allOf", "anyOf",
        "const", "format", "additionalProperties", "unevaluatedProperties", "items",
    };

    // Safe to ignore: documentation only.
    public static readonly HashSet<string> Annotation = new(StringComparer.Ordinal)
    {
        "$schema", "$id", "title", "description", "examples", "$comment", "deprecated",
    };

    // Accepted and deliberately not interpreted: the schema validator and the Rust types
    // enforce them, and the C# output is the same with or without them.
    //
    // `enum` is here rather than in Structural because that is what it actually does. A
    // string enum stays a C# `string` (ADR-0005 section 4-2): the server rejects unknown
    // values, the client must survive them, because the server is deployed first. Generating
    // a C# enum would make one added value drop whole messages on older clients.
    public static readonly HashSet<string> Constraint = new(StringComparer.Ordinal)
    {
        "pattern", "minItems", "uniqueItems", "enum",
    };

    // The basis for integer type selection (ADR-0002 section 4).
    public static readonly HashSet<string> Range = new(StringComparer.Ordinal)
    {
        "minimum", "maximum",
    };

    public static bool Known(string name) =>
        Structural.Contains(name) || Annotation.Contains(name) ||
        Constraint.Contains(name) || Range.Contains(name);
}

sealed class SchemaStore
{
    readonly Dictionary<string, JsonDocument> _docs = new(StringComparer.OrdinalIgnoreCase);
    public string Root { get; }

    public SchemaStore(string contractsRoot) => Root = contractsRoot;

    public JsonElement Document(string file)
    {
        string full = Path.GetFullPath(file);
        if (!_docs.TryGetValue(full, out var doc))
        {
            if (!File.Exists(full)) throw new CodegenException($"schema file not found: {full}");
            try
            {
                doc = JsonDocument.Parse(File.ReadAllText(full));
            }
            catch (JsonException ex)
            {
                throw new CodegenException($"{Display(full)} is not valid JSON: {ex.Message}");
            }
            _docs[full] = doc;
        }
        return doc.RootElement;
    }

    public string Display(string file) =>
        Path.GetRelativePath(Path.GetDirectoryName(Root.TrimEnd(Path.DirectorySeparatorChar)) ?? Root, file)
            .Replace('\\', '/');

    /// <summary>Resolves a $ref of the forms "file.json", "file.json#/$defs/X" and "#/$defs/X",
    /// relative to <paramref name="fromFile"/>. Everything stays offline: only files under
    /// contracts/ are ever opened.</summary>
    public (JsonElement Element, string File) Resolve(string reference, string fromFile, string path)
    {
        int hash = reference.IndexOf('#');
        string filePart = hash < 0 ? reference : reference.Substring(0, hash);
        string pointer = hash < 0 ? "" : reference.Substring(hash + 1);

        string targetFile = filePart.Length == 0
            ? Path.GetFullPath(fromFile)
            : Path.GetFullPath(Path.Combine(Path.GetDirectoryName(Path.GetFullPath(fromFile)), filePart));

        if (!targetFile.StartsWith(Root, StringComparison.OrdinalIgnoreCase))
            throw new CodegenException($"{path}: $ref '{reference}' escapes the contracts directory");

        JsonElement element = Document(targetFile);
        if (pointer.Length == 0) return (element, targetFile);
        if (pointer[0] != '/')
            throw new CodegenException($"{path}: $ref '{reference}' has a non-pointer fragment");

        foreach (string rawToken in pointer.Substring(1).Split('/'))
        {
            string token = rawToken.Replace("~1", "/").Replace("~0", "~");
            if (element.ValueKind != JsonValueKind.Object || !element.TryGetProperty(token, out var next))
                throw new CodegenException($"{path}: $ref '{reference}' does not resolve (stopped at '{token}')");
            element = next;
        }
        return (element, targetFile);
    }

    /// <summary>Expands $ref and validates keywords, producing one keyword map. Sibling
    /// keywords written next to a $ref override the target's (JSON Schema 2020-12).</summary>
    public Dictionary<string, Kw> Flatten(JsonElement schema, string file, string path)
    {
        if (schema.ValueKind != JsonValueKind.Object)
            throw new CodegenException($"{path}: expected a schema object, found {schema.ValueKind}");

        var map = new Dictionary<string, Kw>(StringComparer.Ordinal);

        if (schema.TryGetProperty("$ref", out var refElement))
        {
            string reference = refElement.GetString()
                ?? throw new CodegenException($"{path}/$ref: must be a string");
            var (target, targetFile) = Resolve(reference, file, path);
            foreach (var kv in Flatten(target, targetFile, path + " -> " + reference))
                map[kv.Key] = kv.Value;
        }

        foreach (var prop in schema.EnumerateObject())
        {
            if (prop.Name == "$ref") continue;
            if (!Keywords.Known(prop.Name))
                throw new CodegenException(
                    $"{path}/{prop.Name}: unsupported JSON Schema keyword. Extend the generator " +
                    $"before using it in a contract (ADR-0002 section 4 keyword allowlist).");
            map[prop.Name] = new Kw(prop.Value, file);
        }

        return map;
    }
}

// ---------------------------------------------------------------------------
// Model
// ---------------------------------------------------------------------------

sealed class ObjModel
{
    public string ClassName;
    public string Summary;
    public List<PropModel> Props = new();
    public List<ObjModel> Nested = new();
}

sealed class PropModel
{
    public string JsonName;
    public string CsName;
    public string CsType;        // int / uint / long / bool / string / System.Guid, plus "?" when nullable value type
    public bool Nullable;
    public string RequiredMode;  // Always | AllowNull | Default
    public string ConstName;     // null when the property has no const
    public string ConstLiteral;
    public string Doc;
}

// ---------------------------------------------------------------------------
// Generator
// ---------------------------------------------------------------------------

sealed class Generator
{
    const string Namespace = "Starfall.Contracts.Generated";

    readonly SchemaStore _store;
    readonly string _contracts;

    public Generator(string contractsRoot)
    {
        _contracts = contractsRoot;
        _store = new SchemaStore(contractsRoot);
    }

    public SortedDictionary<string, string> Generate()
    {
        var registryPath = Path.Combine(_contracts, "registry", "types.json");
        var registry = _store.Document(registryPath);

        if (!registry.TryGetProperty("types", out var types) || types.ValueKind != JsonValueKind.Array)
            throw new CodegenException("registry/types.json has no 'types' array");

        var files = new SortedDictionary<string, string>(StringComparer.Ordinal);
        var byName = new SortedDictionary<string, (string Class, int Version)>(StringComparer.Ordinal);

        foreach (var entry in types.EnumerateArray())
        {
            string name = Req(entry, "name", "registry entry");
            string kind = Req(entry, "kind", name);
            string schemaRel = Req(entry, "schema", name);
            string status = entry.TryGetProperty("status", out var s) ? s.GetString() : "active";
            if (status == "deprecated") continue;
            if (kind is "rest" or "data")
                throw new CodegenException($"{name}: kind '{kind}' is not supported by the generator yet");

            int schemaVersion = entry.TryGetProperty("schema_version", out var v) ? v.GetInt32() : 1;

            string schemaFile = Path.GetFullPath(Path.Combine(_contracts, schemaRel));
            var model = BuildType(name, schemaFile, schemaVersion, entry);
            files[model.ClassName + ".cs"] = Render(model, schemaRel);
            byName[name] = (model.ClassName, schemaVersion);
        }

        if (byName.Count == 0) throw new CodegenException("registry has no active types");
        files["ContractTypes.cs"] = RenderRegistry(byName);
        return files;
    }

    static string Req(JsonElement e, string field, string who) =>
        e.TryGetProperty(field, out var value) && value.ValueKind == JsonValueKind.String
            ? value.GetString()
            : throw new CodegenException($"{who}: registry field '{field}' is missing or not a string");

    ObjModel BuildType(string typeName, string schemaFile, int schemaVersion, JsonElement registryEntry)
    {
        string path = _store.Display(schemaFile);
        var flat = _store.Flatten(_store.Document(schemaFile), schemaFile, path);

        string className = flat.TryGetValue("title", out var title) && title.Value.ValueKind == JsonValueKind.String
            ? title.Value.GetString()
            : Pascal(typeName);

        var props = new Dictionary<string, Dictionary<string, Kw>>(StringComparer.Ordinal);
        var required = new HashSet<string>(StringComparer.Ordinal);
        CollectObject(flat, props, required, path);

        var model = new ObjModel
        {
            ClassName = className,
            Summary = $"{typeName} (schema_version {schemaVersion}). Generated from contracts/{path.Substring(path.IndexOf('/') + 1)}; do not edit.",
        };

        BuildProperties(model, props, required, path);

        // The registry, the schema constant and the generated code must agree, or a rename
        // silently produces a DTO that talks about a type nobody dispatches.
        var discriminator = model.Props.FirstOrDefault(p =>
            p.JsonName is "command_type" or "message_type" or "event_type");
        if (discriminator is null)
            throw new CodegenException($"{typeName}: schema has no command_type/message_type/event_type property");
        if (discriminator.ConstLiteral != Quote(typeName))
            throw new CodegenException(
                $"{typeName}: schema type constant is {discriminator.ConstLiteral ?? "absent"}, expected {Quote(typeName)}");

        var version = model.Props.FirstOrDefault(p => p.JsonName == "schema_version");
        if (version?.ConstLiteral != schemaVersion.ToString(CultureInfo.InvariantCulture))
            throw new CodegenException(
                $"{typeName}: schema_version const is {version?.ConstLiteral ?? "absent"}, registry says {schemaVersion}");

        return model;
    }

    void CollectObject(
        Dictionary<string, Kw> flat,
        Dictionary<string, Dictionary<string, Kw>> props,
        HashSet<string> required,
        string path)
    {
        // allOf first (the envelope), then the type's own properties, which override.
        if (flat.TryGetValue("allOf", out var allOf))
        {
            if (allOf.Value.ValueKind != JsonValueKind.Array)
                throw new CodegenException($"{path}/allOf: must be an array");
            int i = 0;
            foreach (var sub in allOf.Value.EnumerateArray())
                CollectObject(_store.Flatten(sub, allOf.File, $"{path}/allOf/{i}"), props, required, $"{path}/allOf/{i++}");
        }

        if (flat.TryGetValue("required", out var req))
            foreach (var r in req.Value.EnumerateArray())
                required.Add(r.GetString());

        if (flat.TryGetValue("properties", out var ps))
        {
            foreach (var p in ps.Value.EnumerateObject())
            {
                var one = _store.Flatten(p.Value, ps.File, $"{path}/properties/{p.Name}");
                if (props.TryGetValue(p.Name, out var existing))
                {
                    // A type schema may NARROW an envelope field: SESSION_OPENED and
                    // SESSION_CLOSED redeclare actor_id as a plain UuidV7 where the envelope
                    // says "UuidV7 or null". Merging keyword by keyword would leave the
                    // inherited anyOf sitting next to the narrowed concrete type, and
                    // Normalize reads anyOf first, so the narrowing would be silently ignored
                    // and the DTO would keep accepting null (ADR-0005 section 4-3, U-2).
                    // A concrete `type` from the overriding schema REPLACES the inherited
                    // composition instead of joining it. The widening direction still works:
                    // an override that brings its own anyOf keeps it and Normalize picks it up.
                    if (one.ContainsKey("type") && !one.ContainsKey("anyOf"))
                        existing.Remove("anyOf");
                    foreach (var kv in one) existing[kv.Key] = kv.Value;
                }
                else
                    props[p.Name] = one;
            }
        }
    }

    void BuildProperties(
        ObjModel model,
        Dictionary<string, Dictionary<string, Kw>> props,
        HashSet<string> required,
        string path)
    {
        foreach (string jsonName in props.Keys.OrderBy(k => k, StringComparer.Ordinal))
        {
            var map = props[jsonName];
            string propPath = $"{path}/properties/{jsonName}";
            var shape = Normalize(map, propPath);

            string csName = Pascal(jsonName);
            bool isRequired = required.Contains(jsonName);

            var prop = new PropModel
            {
                JsonName = jsonName,
                CsName = csName,
                Nullable = shape.Nullable,
                CsType = shape.Nested is not null
                    ? shape.Nested.ClassName
                    : shape.CsType + (shape.Nullable && IsValueType(shape.CsType) ? "?" : ""),
                RequiredMode = !isRequired ? "Default" : shape.Nullable ? "AllowNull" : "Always",
                Doc = shape.Doc,
            };

            if (shape.Const is JsonElement constant)
            {
                prop.ConstName = csName + "Const";
                prop.ConstLiteral = constant.ValueKind switch
                {
                    JsonValueKind.String => Quote(constant.GetString()),
                    JsonValueKind.Number => constant.GetRawText(),
                    JsonValueKind.True => "true",
                    JsonValueKind.False => "false",
                    _ => throw new CodegenException($"{propPath}/const: unsupported constant kind {constant.ValueKind}"),
                };
            }

            if (shape.Nested is not null) model.Nested.Add(shape.Nested);
            model.Props.Add(prop);
        }

        model.Nested = model.Nested.OrderBy(n => n.ClassName, StringComparer.Ordinal).ToList();
    }

    sealed class Shape
    {
        public string CsType;
        public bool Nullable;
        public ObjModel Nested;
        public JsonElement? Const;
        public string Doc;
    }

    Shape Normalize(Dictionary<string, Kw> map, string path)
    {
        string doc = map.TryGetValue("description", out var d) && d.Value.ValueKind == JsonValueKind.String
            ? d.Value.GetString()
            : null;

        // anyOf is only used for "X or null" (ADR-0002 section 1: envelope fields always exist,
        // absent values are explicit nulls).
        if (map.TryGetValue("anyOf", out var anyOf))
        {
            var branches = anyOf.Value.EnumerateArray().ToList();
            var nonNull = new List<JsonElement>();
            bool sawNull = false;
            foreach (var branch in branches)
            {
                if (branch.ValueKind == JsonValueKind.Object &&
                    branch.TryGetProperty("type", out var bt) &&
                    bt.ValueKind == JsonValueKind.String && bt.GetString() == "null")
                    sawNull = true;
                else
                    nonNull.Add(branch);
            }
            if (!sawNull || nonNull.Count != 1)
                throw new CodegenException(
                    $"{path}/anyOf: only the 'X or null' shape is supported (found {branches.Count} branches, null={sawNull})");

            var inner = _store.Flatten(nonNull[0], anyOf.File, $"{path}/anyOf/0");
            foreach (var kv in map)
                if (kv.Key != "anyOf") inner[kv.Key] = kv.Value;

            var innerShape = Normalize(inner, $"{path}/anyOf/0");
            innerShape.Nullable = true;
            innerShape.Doc ??= doc;
            return innerShape;
        }

        JsonElement? constant = map.TryGetValue("const", out var c) ? c.Value : null;

        if (!map.TryGetValue("type", out var typeKw) || typeKw.Value.ValueKind != JsonValueKind.String)
            throw new CodegenException($"{path}: no 'type' keyword — the generator cannot infer a C# type");

        string type = typeKw.Value.GetString();
        switch (type)
        {
            case "string":
            {
                bool isUuid = map.TryGetValue("format", out var f) &&
                              f.Value.ValueKind == JsonValueKind.String && f.Value.GetString() == "uuid";
                // RealTime / GameTime deliberately stay strings: putting a game calendar date
                // (year 3827) into DateTime puts game time onto the real time axis.
                return new Shape { CsType = isUuid ? "System.Guid" : "string", Const = constant, Doc = doc };
            }
            case "integer":
            {
                if (!map.TryGetValue("minimum", out var min) || !map.TryGetValue("maximum", out var max))
                    throw new CodegenException(
                        $"{path}: integer without both 'minimum' and 'maximum'. The C# type is chosen from the " +
                        $"declared range (ADR-0002 section 4), so an unbounded integer has no mapping.");
                return new Shape
                {
                    CsType = Narrowest(min.Value.GetInt64(), max.Value.GetInt64(), path),
                    Const = constant,
                    Doc = doc,
                };
            }
            case "boolean":
                return new Shape { CsType = "bool", Const = constant, Doc = doc };
            case "object":
            {
                var nestedProps = new Dictionary<string, Dictionary<string, Kw>>(StringComparer.Ordinal);
                var nestedRequired = new HashSet<string>(StringComparer.Ordinal);
                CollectObject(map, nestedProps, nestedRequired, path);

                string className = map.TryGetValue("title", out var t) && t.Value.ValueKind == JsonValueKind.String
                    ? t.Value.GetString()
                    : throw new CodegenException($"{path}: nested object needs a 'title' to name the C# class");

                var nested = new ObjModel { ClassName = className, Summary = doc };
                BuildProperties(nested, nestedProps, nestedRequired, path);
                return new Shape { Nested = nested, Doc = doc };
            }
            default:
                throw new CodegenException($"{path}/type: '{type}' is not supported by the generator");
        }
    }

    static string Narrowest(long min, long max, string path)
    {
        if (min > max) throw new CodegenException($"{path}: minimum {min} is greater than maximum {max}");
        if (min >= int.MinValue && max <= int.MaxValue) return "int";
        if (min >= 0 && max <= uint.MaxValue) return "uint";
        return "long"; // ulong is never used: it would loosen the +/-2^53-1 rule on the C# side only.
    }

    static bool IsValueType(string csType) =>
        csType is "int" or "uint" or "long" or "bool" or "System.Guid";

    // -----------------------------------------------------------------------
    // Rendering
    // -----------------------------------------------------------------------

    static string Render(ObjModel model, string schemaRel)
    {
        var sb = new StringBuilder();
        Header(sb, schemaRel, "Newtonsoft.Json");
        sb.Append("namespace ").Append(Namespace).Append("\n{\n");
        RenderClass(sb, model, 1);
        sb.Append("}\n");
        return sb.ToString();
    }

    static void RenderClass(StringBuilder sb, ObjModel model, int depth)
    {
        string pad = new string(' ', depth * 4);

        if (!string.IsNullOrEmpty(model.Summary))
        {
            sb.Append(pad).Append("/// <summary>").Append(Escape(model.Summary)).Append("</summary>\n");
        }
        sb.Append(pad).Append("public sealed class ").Append(model.ClassName).Append('\n');
        sb.Append(pad).Append("{\n");

        string inner = pad + "    ";
        bool first = true;

        foreach (var p in model.Props.Where(p => p.ConstName is not null))
        {
            if (!first) sb.Append('\n');
            first = false;
            sb.Append(inner).Append("public const ")
              .Append(p.CsType == "System.Guid" ? "string" : p.CsType).Append(' ')
              .Append(p.ConstName).Append(" = ").Append(p.ConstLiteral).Append(";\n");
        }

        foreach (var p in model.Props)
        {
            if (!first) sb.Append('\n');
            first = false;

            if (!string.IsNullOrEmpty(p.Doc))
                sb.Append(inner).Append("/// <summary>").Append(Escape(p.Doc)).Append("</summary>\n");

            sb.Append(inner).Append("[JsonProperty(\"").Append(p.JsonName).Append("\", Required = Required.")
              .Append(p.RequiredMode);
            if (p.RequiredMode == "Default")
                sb.Append(", NullValueHandling = NullValueHandling.Ignore");
            sb.Append(")]\n");

            sb.Append(inner).Append("public ").Append(p.CsType).Append(' ').Append(p.CsName)
              .Append(" { get; set; }");
            if (p.ConstName is not null) sb.Append(" = ").Append(p.ConstName).Append(';');
            sb.Append('\n');
        }

        foreach (var nested in model.Nested)
        {
            sb.Append('\n');
            RenderClass(sb, nested, depth + 1);
        }

        sb.Append(pad).Append("}\n");
    }

    static string RenderRegistry(SortedDictionary<string, (string Class, int Version)> byName)
    {
        var sb = new StringBuilder();
        Header(sb, "registry/types.json", "System", "System.Collections.Generic");
        sb.Append("namespace ").Append(Namespace).Append("\n{\n");
        sb.Append("    /// <summary>Registry type name to generated DTO. Message dispatch and the\n");
        sb.Append("    /// fixture tests look types up here instead of hard-coding a switch.</summary>\n");
        sb.Append("    public static class ContractTypes\n    {\n");

        sb.Append("        /// <summary>Registry name (SCREAMING_SNAKE_CASE) to DTO type.</summary>\n");
        sb.Append("        public static readonly IReadOnlyDictionary<string, Type> ByName =\n");
        sb.Append("            new Dictionary<string, Type>(StringComparer.Ordinal)\n            {\n");
        foreach (var kv in byName)
            sb.Append("                { \"").Append(kv.Key).Append("\", typeof(").Append(kv.Value.Class).Append(") },\n");
        sb.Append("            };\n\n");

        sb.Append("        /// <summary>Registry name to the schema_version this build was generated from.</summary>\n");
        sb.Append("        public static readonly IReadOnlyDictionary<string, int> SchemaVersionByName =\n");
        sb.Append("            new Dictionary<string, int>(StringComparer.Ordinal)\n            {\n");
        foreach (var kv in byName)
            sb.Append("                { \"").Append(kv.Key).Append("\", ").Append(kv.Value.Version).Append(" },\n");
        sb.Append("            };\n");

        sb.Append("    }\n}\n");
        return sb.ToString();
    }

    // Only the usings a file actually needs: an unnecessary using is a warning under some
    // analyzer settings, and AC-8 asks for zero warnings under Assets/_Project/**.
    static void Header(StringBuilder sb, string source, params string[] usings)
    {
        sb.Append("// <auto-generated>\n");
        sb.Append("//     Generated by tools/codegen/ContractsCodegen.cs from contracts/").Append(source).Append(".\n");
        sb.Append("//     Do not edit. Re-run the generator; `--check` fails on hand edits.\n");
        sb.Append("// </auto-generated>\n");
        sb.Append("#nullable disable\n");
        foreach (string u in usings) sb.Append("using ").Append(u).Append(";\n");
        sb.Append('\n');
    }

    static string Quote(string s) => "\"" + s.Replace("\\", "\\\\").Replace("\"", "\\\"") + "\"";

    static string Escape(string s) =>
        s.Replace("&", "&amp;").Replace("<", "&lt;").Replace(">", "&gt;").Replace("\n", " ").Trim();

    static string Pascal(string snake)
    {
        var sb = new StringBuilder();
        foreach (string part in snake.Split('_', StringSplitOptions.RemoveEmptyEntries))
        {
            sb.Append(char.ToUpperInvariant(part[0]));
            if (part.Length > 1) sb.Append(part.Substring(1).ToLowerInvariant());
        }
        return sb.ToString();
    }
}
