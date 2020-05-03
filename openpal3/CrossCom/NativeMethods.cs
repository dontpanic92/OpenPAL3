using System;
using System.Collections.Generic;
using System.Runtime.InteropServices;
using System.Text;

namespace CrossCom
{
    class NativeMethods
    {
        [DllImport("opengb")]
        public static extern long DllGetClassObject([MarshalAs(UnmanagedType.LPStruct)] Guid rclsid, [MarshalAs(UnmanagedType.LPStruct)] Guid riid, out IntPtr pointer);
    }
}
