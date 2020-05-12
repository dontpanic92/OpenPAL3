// <copyright file="IUnknownCcw.cs">
// Copyright (c) Shengqiu Li and OpenPAL3 Developers. All rights reserved.
// Licensed under the GPLv3 license. See LICENSE file in the project root for full license information.
// </copyright>

namespace CrossCom
{
    using System;
    using System.Runtime.InteropServices;
    using CrossCom.Metadata;

    /// <summary>
    /// The com callable wrapper for <see cref="IUnknown"/>.
    /// Instances need to be explictly freed with <see cref="Dispose"/>.
    /// </summary>
    public class IUnknownCcw : IDisposable
    {
        private static readonly GCHandle VirtualTableHandle;
        private readonly GCHandle self;

        /// <summary>
        /// Initializes static members of the <see cref="IUnknownCcw"/> class.
        /// </summary>
        static IUnknownCcw()
        {
            VirtualTable = new IUnknownVirtualTable()
            {
                QueryInterfacePointer = Marshal.GetFunctionPointerForDelegate<IUnknown._QueryInterface>(QueryInterface),
                AddRefPointer = Marshal.GetFunctionPointerForDelegate<IUnknown._AddRef>(AddRef),
                ReleasePointer = Marshal.GetFunctionPointerForDelegate<IUnknown._Release>(Release),
            };

            VirtualTableHandle = GCHandle.Alloc(VirtualTable, GCHandleType.Pinned);
        }

        /// <summary>
        /// Initializes a new instance of the <see cref="IUnknownCcw"/> class.
        /// </summary>
        public IUnknownCcw()
            : this(VirtualTableHandle.AddrOfPinnedObject())
        {
        }

        /// <summary>
        /// Initializes a new instance of the <see cref="IUnknownCcw"/> class.
        /// </summary>
        /// <param name="virtualTable">The virtual table.</param>
        protected IUnknownCcw(IntPtr virtualTable)
        {
            this.self = GCHandle.Alloc(virtualTable, GCHandleType.Pinned);
        }

        /// <summary>
        /// Gets the virtual table.
        /// </summary>
        public static IUnknownVirtualTable VirtualTable { get; }

        /// <summary>
        /// Get raw COM ptr.
        /// </summary>
        /// <returns>The raw COM ptr.</returns>
        public IntPtr GetComPtr()
        {
            return this.self.AddrOfPinnedObject();
        }

        /// <inheritdoc/>
        public void Dispose()
        {
            this.self.Free();
        }

        /// <summary>
        /// Get the managed object through a raw COM ptr.
        /// </summary>
        /// <param name="self">The raw COM ptr.</param>
        /// <returns>The object found.</returns>
        protected static IUnknown GetObject(IntPtr self)
        {
            if (Unknown.ObjectCache.TryGetValue(self, out var obj))
            {
                return obj;
            }

            return new IUnknownRcw(self);
        }

        private static long QueryInterface(IntPtr self, Guid guid, out IntPtr retval)
        {
            retval = (GetObject(self) as IComObject) !.GetComPtr(Type.GetTypeFromCLSID(guid));
            if (retval == IntPtr.Zero)
            {
                return 1;
            }

            return 0;
        }

        private static long AddRef(IntPtr self)
        {
            return (GetObject(self) as IUnknownInternal) !.AddRef();
        }

        private static long Release(IntPtr self)
        {
            return (GetObject(self) as IUnknownInternal) !.Release();
        }

        /// <summary>
        /// VirtualTable for <see cref="IUnknownCcw"/>.
        /// </summary>
        [StructLayout(LayoutKind.Sequential)]
        public struct IUnknownVirtualTable
        {
            /// <summary>
            /// Pointer to the QueryInterface function.
            /// </summary>
            public IntPtr QueryInterfacePointer;

            /// <summary>
            /// Pointer to the AddRef function.
            /// </summary>
            public IntPtr AddRefPointer;

            /// <summary>
            /// Pointer to the Release function.
            /// </summary>
            public IntPtr ReleasePointer;
        }
    }
}
