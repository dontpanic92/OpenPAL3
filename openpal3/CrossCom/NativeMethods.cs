// <copyright file="NativeMethods.cs">
// Copyright (c) Shengqiu Li and OpenPAL3 Developers. All rights reserved.
// Licensed under the GPLv3 license. See LICENSE file in the project root for full license information.
// </copyright>

namespace CrossCom
{
    using System;
    using System.Runtime.InteropServices;

    /// <summary>
    /// Native methods.
    /// </summary>
    internal class NativeMethods
    {
        /// <summary>
        /// Gets the class factory object.
        /// </summary>
        /// <param name="rclsid">The class id of the factory.</param>
        /// <param name="riid">The interface id.</param>
        /// <param name="pointer">The factory object.</param>
        /// <returns>HResult representing the operation status.</returns>
        [DllImport("opengb")]
        public static extern long DllGetClassObject([MarshalAs(UnmanagedType.LPStruct)] Guid rclsid, [MarshalAs(UnmanagedType.LPStruct)] Guid riid, out IntPtr pointer);
    }
}
