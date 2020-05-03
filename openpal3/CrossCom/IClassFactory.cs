using CrossCom.Attributes;
using System;
using System.Collections.Generic;
using System.Text;

namespace CrossCom
{
    [CrossComInterfaceImport("00000001-0000-0000-C000-000000000046", typeof(ClassFactory))]
    public interface IClassFactory : IUnknown
    {
        [CrossComMethod]
        delegate long _CreateInstance(IntPtr self, IntPtr outer, Guid guid, out IntPtr retval);

        [CrossComMethod]
        delegate long _LockServer(IntPtr self);

        TInterface CreateInstance<TInterface>() where TInterface : class, IUnknown;
    }
}
