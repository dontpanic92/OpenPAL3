using CrossCom;
using System;
using System.Collections.Generic;
using System.Text;

namespace OpenGB.Runtime
{
    class FactoryObject : IUnknownObject, IFactory
    {
        public FactoryObject(IntPtr ptr)
            : base(ptr)
        {
        }

        public int Echo(int value)
        {
            return this.GetMethod<IFactory._Echo>()(this.GetComPtr(), value);
        }

        public IntPtr LoadOpengbConfig(string name, string env_prefix)
        {
            if (this.GetMethod<IFactory._LoadOpengbConfig>()(this.GetComPtr(), name, env_prefix, out var ptr) == 0)
            {
                return ptr;
            }

            return IntPtr.Zero;
        }
    }
}
