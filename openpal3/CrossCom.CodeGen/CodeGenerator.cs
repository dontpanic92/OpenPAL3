namespace CrossCom.CodeGen
{
    using System;
    using System.Collections.Generic;
    using System.IO;
    using System.Linq;
    using System.Reflection;
    using System.Runtime.InteropServices;
    using HandlebarsDotNet;
    using Newtonsoft.Json;
    using Newtonsoft.Json.Linq;

    public class CodeGenerator
    {
        public const string AutomationSuffix = "_Automation";
        public const string RawSuffix = "_Raw";

        private readonly IdlLib idlLib;
        private readonly Config config;
        private readonly string outputPath;

        public CodeGenerator(string idlJsonPath, string outputPath, string configPath)
        {
            this.outputPath = outputPath;
            this.idlLib = JsonConvert.DeserializeObject<IdlLib>(File.ReadAllText(idlJsonPath));
            this.config = JsonConvert.DeserializeObject<Config>(File.ReadAllText(configPath));

            this.idlLib.Namespace = config.Namespace;
            this.idlLib.Interfaces = this.idlLib.Interfaces
                .Where(t => !this.config.IgnoreInterface.Contains(t.Name))
                .Where(t => !t.OriginalName.EndsWith(RawSuffix))
                .ToList();
            this.idlLib.CoClasses = this.idlLib.CoClasses
                .Where(t => !this.config.IgnoreClass.Contains(t.Name))
                .ToList();
        }

        public void Generate()
        {
            var sourceTemplate = Handlebars.Compile(File.ReadAllText(Path.Combine(Path.GetDirectoryName(Assembly.GetExecutingAssembly().Location), "Source.cs.hbt")));

            Directory.CreateDirectory(this.outputPath);

            var result = sourceTemplate(this.idlLib);
            File.WriteAllText(Path.Combine(this.outputPath, "Source.cs"), result);
        }

        private class Config
        {
            public string Namespace { get; set; }

            public IList<string> IgnoreClass { get; set; } = new List<string>();

            public IList<string> IgnoreInterface { get; set; } = new List<string>();
        }

        private class IdlLib
        {
            public IdlLib(string libName, string libId, IList<Interface> interfaces, IList<CoClass> coClasses)
            {
                this.Name = libName;
                this.Id = libId;
                this.Interfaces = interfaces;
                this.CoClasses = coClasses;
            }

            public string Name { get; }

            public string Id { get; }

            public IList<Interface> Interfaces { get; set; }

            public IList<CoClass> CoClasses { get; set; }

            public string Namespace { get; set; }
        }

        private class Interface
        {
            public Interface(string name, string @base, string iid, IList<Method> methods)
            {
                this.OriginalName = name;
                this.Base = @base;
                this.InterfaceId = iid;
                this.Methods = methods.OrderBy(m => m.Index).ToList();

                if (name.EndsWith(RawSuffix))
                {
                    name = name[0..^RawSuffix.Length];
                }
                else if (name.EndsWith(AutomationSuffix))
                {
                    name = name[0..^AutomationSuffix.Length];
                }

                this.Name = name;
            }

            public string OriginalName { get; }

            public string Base { get; }

            public string InterfaceId { get; }

            public IList<Method> Methods { get; }

            public string Name { get; }
        }

        private class Method
        {
            public Method(string name, int idx, string ret_type, IList<Arg> args)
            {
                this.Name = name;
                this.Index = idx;
                this.OriginalReturnType = ret_type;
                this.Arguments = args;

                var type = new WrappedType(ret_type, string.Empty);
                this.RawReturnType = type.GetRawTypeString(false);
                this.ManagedReturnType = type.GetManagedTypeString();
                this.MarshalReturnType = type.MarshalAs != null;
                this.MarshalReturnTypeAs = type.MarshalAs != null ? type.MarshalAs.ToString() : string.Empty;
                this.ReturnVoid = (this.RawReturnType == "void");
            }

            public string Name { get; }

            public int Index { get; }

            public string OriginalReturnType { get; }

            public string RawReturnType { get; }

            public string ManagedReturnType { get; }

            public bool MarshalReturnType { get; }

            public string MarshalReturnTypeAs { get; }

            public bool ReturnVoid { get; }

            public IList<Arg> Arguments { get; }
        }

        private class Arg
        {
            public Arg(string name, string arg_type, string attributes)
            {
                this.Name = name;
                this.OriginalType = arg_type;
                this.WrappedType = new WrappedType(arg_type, attributes);
                this.RawTypeWithDecorator = this.WrappedType.GetRawTypeString(true);
                this.ManagedTypeWithDecorator = this.WrappedType.GetManagedTypeString();
            }

            public string OriginalType { get; }

            public string Name { get; }

            public string RawTypeWithDecorator { get; }

            public string ManagedTypeWithDecorator { get; }

            public WrappedType WrappedType { get; }
        }

        private class CoClass
        {
            public CoClass(string name, string clsid, IList<string> implement)
            {
                this.Name = name;
                this.ClassId = clsid;
                this.Implement = implement;
            }

            public string Name { get; }

            public string ClassId { get; }

            public IList<string> Implement { get; }
        }
    }
}
