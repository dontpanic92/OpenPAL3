using System;
using System.Collections.Generic;
using System.Linq;
using System.Runtime.InteropServices;
using System.Text;

namespace CrossCom.CodeGen
{
    public class WrappedType
    {
        public string RawType { get; private set; }

        public string ManagedType { get; private set; }

        public UnmanagedType? MarshalAs { get; private set; }

        public bool IsComObject { get; private set; }

        public bool IsOut { get; private set; }

        public bool IsRef { get; private set; }

        public bool IsArray { get; private set; }

        public WrappedType(string type, string attributes)
        {
            var attrList = attributes.Split(',').Select(t => t.Trim()).ToList();
            string baseType = type.Trim('*');
            int indirectionLevel = type.Count(c => c == '*');

            (this.RawType, this.ManagedType, this.IsComObject, this.MarshalAs) = MapType(baseType, ref indirectionLevel);
            if (this.RawType == null)
            {
                throw new NotSupportedException($"Unsupported type: {type}");
            }

            if (attrList.Contains("out"))
            {
                if (indirectionLevel == 0)
                {
                    throw new NotSupportedException("The out argument should be a pointer type");
                }

                this.IsOut = true;
                indirectionLevel--;
            }

            if (indirectionLevel > 0)
            {
                if (attrList.Contains("ref"))
                {
                    if (this.IsOut)
                    {
                        throw new NotSupportedException($"A type cannot be ref and out at the same time: {type}");
                    }

                    this.IsRef = true;
                }
                else
                {
                    this.IsArray = true;
                }

                indirectionLevel--;
            }

            if (indirectionLevel > 0)
            {
                throw new NotSupportedException("Too many indirection levels");
            }
        }

        public string GetRawTypeString(bool withMarshalAsAttr)
        {
            var marshalAsString = ((this.MarshalAs == null) || (!withMarshalAsAttr)) ? string.Empty : $"[MarshalAs(UnmanagedType.{this.MarshalAs})]";
            var type = this.GetTypeString(this.RawType);
            var finalType = $"{marshalAsString} {type}".Trim();
            return finalType;
        }

        public string GetManagedTypeString()
        {
            return this.GetTypeString(this.ManagedType);
        }

        private string GetTypeString(string type)
        {
            var outString = this.IsOut ? "out" : string.Empty;
            var refString = this.IsRef ? "ref" : string.Empty;
            var arrayString = this.IsArray ? "[]" : string.Empty;
            var finalType = $"{outString}{refString} {type}{arrayString}".Trim();
            return finalType;
        }

        private static (string RawType, string ManagedType, bool IsComObject, UnmanagedType? MarshapAs) MapType(string type, ref int indirectionLevel)
        {
            switch (type)
            {
                case "i8":
                    return ("char", "char", false, null);
                case "u8":
                    return ("byte", "byte", false, null);
                case "i16":
                    return ("short", "short", false, null);
                case "u16":
                    return ("ushort", "ushort", false, null);
                case "i32":
                    return ("int", "int", false, null);
                case "u32":
                    return ("uint", "uint", false, null);
                case "i64":
                    return ("long", "long", false, null);
                case "u64":
                    return ("ulong", "ulong", false, null);
                case "f32":
                    return ("float", "float", false, null);
                case "f64":
                    return ("double", "double", false, null);
                case "usize":
                    return ("ulong", "ulong", false, null);
                case "HRESULT":
                    return ("long", "long", false, null);
                case "void":
                    if (indirectionLevel > 0)
                    {
                        indirectionLevel--;
                        return ("IntPtr", "IntPtr", false, null);
                    }

                    return ("void", "void", false, null);
                case "InBSTR":
                case "OutBSTR":
                    return ("string", "string", false, UnmanagedType.BStr);
            }

            if (indirectionLevel == 0)
            {
                throw new NotSupportedException("Not enough indirection level");
            }

            indirectionLevel--;
            var managedType = type.StartsWith('I') ? type : "I" + type;
            managedType = managedType.EndsWith(CodeGenerator.AutomationSuffix) 
                ? managedType[0..^CodeGenerator.AutomationSuffix.Length] 
                : managedType;

            return ("IntPtr", managedType, true, null);
        }
    }
}
