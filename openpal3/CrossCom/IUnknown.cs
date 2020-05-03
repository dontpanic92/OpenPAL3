using CrossCom.Attributes;
using System;
using System.Collections.Generic;
using System.Text;

namespace CrossCom
{
    [CrossComInterfaceImport("00000001-0000-0000-C000-000000000046", typeof(IUnknownObject))]
    public interface IUnknown : IDisposable
    {
        [CrossComMethod]
        delegate long _QueryInterface(IntPtr self, Guid guid, out IntPtr retval);

        [CrossComMethod]
        delegate long _AddRef(IntPtr self);

        [CrossComMethod]
        delegate long _Release(IntPtr self);

        TInterface QueryInterface<TInterface>()
            where TInterface : class, IUnknown;
    }
}
