using CrossCom.Attributes;
using CrossCom.Metadata;
using System;
using System.Collections.Generic;
using System.Runtime.InteropServices;
using System.Text;

namespace CrossCom
{
    public class IUnknownObject : ImportedObject, IUnknown
    {
        public IUnknownObject(IntPtr ptr)
            : base(ptr)
        {
        }

        public long AddRef()
        {
            return this.GetMethod<IUnknown._AddRef>()(this.GetComPtr());
        }

        public TInterface QueryInterface<TInterface>() where TInterface : class, IUnknown
        {
            var result = this.GetMethod<IUnknown._QueryInterface>()(this.GetComPtr(), ImportedInterfaceMetadata<TInterface>.Value.Guid, out var ptr);
            if (result == 0)
            {
                var obj = ObjectActivator<TInterface>.CreateInstance(ptr);
                return obj;
            }

            return null;
        }

        public long Release()
        {
            return this.GetMethod<IUnknown._Release>()(this.GetComPtr());
        }
    }
}
