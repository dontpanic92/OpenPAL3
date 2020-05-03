namespace CrossCom.CodeGen
{
    using System;
    using System.Collections.Generic;
    using System.IO;
    using System.Linq;
    using System.Reflection;
    using HandlebarsDotNet;
    using Newtonsoft.Json;
    using Newtonsoft.Json.Linq;

    public class CodeGenerator
    {
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
                .Where(t => !t.OriginalName.EndsWith("_Raw"))
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
                this.Methods = methods;

                if (name.EndsWith("_Raw"))
                {
                    name = name.Substring(0, name.Length - 4);
                }
                else if (name.EndsWith("_Automation"))
                {
                    name = name.Substring(0, name.Length - 11);
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
            public Method(string name, int idx, string retType, IList<Arg> args)
            {
                this.Name = name;
                this.Index = idx;
                this.ReturnType = retType;
                this.Arguments = args;
            }

            public string Name { get; }

            public int Index { get; }

            public string ReturnType { get; }

            public IList<Arg> Arguments { get; }
        }

        private class Arg
        {
            public Arg(string name, string argType, string attributes)
            {
                this.Name = name;
                this.Type = argType;
            }

            public string Name { get; }

            public string Type { get; }
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
