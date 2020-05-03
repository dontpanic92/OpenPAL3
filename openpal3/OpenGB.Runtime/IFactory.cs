using System;
using System.Collections.Generic;
using System.Runtime.InteropServices;
using System.Text;
using CrossCom;
using CrossCom.Attributes;

namespace OpenGB.Runtime
{
    [CrossComInterfaceImport("2D99E8BC-39DE-3C28-4658-15E364F1B959", typeof(FactoryObject))]
    public interface IFactory : IUnknown
    {
        [CrossComMethod]
        delegate long _LoadOpengbConfig(IntPtr self, [MarshalAs(UnmanagedType.BStr)] string name, [MarshalAs(UnmanagedType.BStr)] string env_prefix, out IntPtr ptr);

        [CrossComMethod]
        delegate int _Echo(IntPtr self, int value);

        IntPtr LoadOpengbConfig(string name, string env_prefix);

        int Echo(int value);
    }
}
