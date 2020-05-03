using System;
using System.Collections.Generic;
using System.Text;

namespace CrossCom.Attributes
{
    public class CrossComObjectImport : Attribute
    {
        public CrossComObjectImport(string guid)
        {
            this.Guid = guid;
        }

        public string Guid { get; }
    }
}
