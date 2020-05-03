using System;
using System.Collections.Generic;
using System.Linq;
using System.Text;

namespace CrossCom.Attributes
{
    [AttributeUsage(AttributeTargets.Interface, Inherited = false, AllowMultiple = false)]
    public class CrossComInterfaceImport: Attribute
    {
        public CrossComInterfaceImport(string guid, Type implementation)
        {
            this.Guid = guid;
            this.Implementation = implementation;
        }

        public string Guid { get; }

        public Type Implementation { get; }
    }
}
