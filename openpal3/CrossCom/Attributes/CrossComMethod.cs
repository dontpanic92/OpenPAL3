using System;
using System.Collections.Generic;
using System.Runtime.CompilerServices;
using System.Text;

namespace CrossCom.Attributes
{
    public class CrossComMethod : Attribute
    {
        public CrossComMethod([CallerLineNumber] int order = 0)
        {
            this.Order = order;
        }

        public int Order { get; }
    }
}
