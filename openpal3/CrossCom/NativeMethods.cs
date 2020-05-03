using System;
using System.Collections.Generic;
using System.Runtime.InteropServices;
using System.Text;

namespace CrossCom
{
    class NativeMethods
    {
        [DllImport("opengb.dll")]
        public static extern long DllGetClassObject(Guid rclsid, Guid riid, out IntPtr pointer);
    }
}
