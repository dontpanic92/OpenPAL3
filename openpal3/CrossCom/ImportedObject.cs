// <copyright file="ImportedObject.cs">
// Copyright (c) Shengqiu Li and OpenPAL3 Developers. All rights reserved.
// Licensed under the GPLv3 license. See LICENSE file in the project root for full license information.
// </copyright>

namespace CrossCom
{
    using System;
    using System.Runtime.InteropServices;
    using CrossCom.Metadata;

    /// <summary>
    /// The base class for all the implementations of imported interfaces.
    /// </summary>
    public abstract class ImportedObject
    {
        private readonly IntPtr self;
        private readonly IntPtr vtable;
        private readonly Delegate[] methods;

        /// <summary>
        /// Initializes a new instance of the <see cref="ImportedObject"/> class.
        /// </summary>
        /// <param name="ptr">The COM object ptr.</param>
        public ImportedObject(IntPtr ptr)
        {
            this.self = ptr;
            this.vtable = Marshal.ReadIntPtr(ptr);
            this.methods = new Delegate[InterfaceObjectMetadata.GetValue(this.GetType()).VirtualTablesize];
        }

        /// <summary>
        /// Gets the COM method delegate.
        /// </summary>
        /// <typeparam name="TDelegate">The delegate type.</typeparam>
        /// <returns>The delegate.</returns>
        public TDelegate GetMethod<TDelegate>()
            where TDelegate : Delegate
        {
            var index = VirtualMethodMetadata<TDelegate>.Value.Index;
            if (this.methods[index] == null)
            {
                var method = new IntPtr(this.vtable.ToInt64() + (index * IntPtr.Size));
                this.methods[index] = Marshal.GetDelegateForFunctionPointer<TDelegate>(Marshal.ReadIntPtr(method));
            }

            return (TDelegate)this.methods[index];
        }

        /// <summary>
        /// Gets the raw COM object ptr.
        /// </summary>
        /// <returns>The raw COM object ptr.</returns>
        public IntPtr GetComPtr()
        {
            return this.self;
        }
    }
}
