// <copyright file="IUnknownRcw.cs">
// Copyright (c) Shengqiu Li and OpenPAL3 Developers. All rights reserved.
// Licensed under the GPLv3 license. See LICENSE file in the project root for full license information.
// </copyright>

namespace CrossCom
{
    using System;
    using System.Runtime.InteropServices;
    using CrossCom.Activators;
    using CrossCom.Metadata;

    /// <summary>
    /// The runtime callable wrapper for <see cref="IUnknown"/>.
    /// This is the base class for all the imported objects.
    /// </summary>
    public class IUnknownRcw : IUnknown, IUnknownInternal, IComObject
    {
        private readonly IntPtr self;
        private readonly IntPtr vtable;
        private readonly Delegate[] methods;
        private bool disposedValue = false;

        /// <summary>
        /// Initializes a new instance of the <see cref="IUnknownRcw"/> class.
        /// </summary>
        /// <param name="ptr">The raw COM ptr.</param>
        public IUnknownRcw(IntPtr ptr)
        {
            this.self = ptr;
            this.vtable = Marshal.ReadIntPtr(ptr);
            this.methods = new Delegate[RcwTypeMetadata.GetValue(this.GetType()).VirtualTableSize];

            this.AddRef();
        }

        /// <summary>
        /// Finalizes an instance of the <see cref="IUnknownRcw"/> class.
        /// </summary>
        ~IUnknownRcw()
        {
            // Do not change this code. Put cleanup code in Dispose(bool disposing) above.
            this.Dispose(false);
        }

        /// <summary>
        /// Create a new <see cref="IUnknownRcw"/> object.
        /// </summary>
        /// <param name="ptr">The raw COM ptr.</param>
        /// <returns>The object created.</returns>
        public static IUnknown Create(IntPtr ptr)
        {
            return new IUnknownRcw(ptr);
        }

        /// <inheritdoc/>
        public TInterface? QueryInterface<TInterface>()
            where TInterface : class, IUnknown
        {
            var result = this.GetMethod<IUnknown._QueryInterface>()(this.GetComPtr(typeof(TInterface)), typeof(TInterface).GUID, out var ptr);
            if (result == 0)
            {
                var obj = RcwActivator<TInterface>.CreateInstance(ptr);
                return obj;
            }

            return null;
        }

        /// <inheritdoc/>
        public IntPtr GetComPtr(Type interfaceType)
        {
            return this.self;
        }

        /// <inheritdoc/>
        public long AddRef()
        {
            return this.GetMethod<IUnknown._AddRef>()(this.GetComPtr(typeof(IUnknown)));
        }

        /// <inheritdoc/>
        public long Release()
        {
            return this.GetMethod<IUnknown._Release>()(this.GetComPtr(typeof(IUnknown)));
        }

        /// <inheritdoc/>
        public void Dispose()
        {
            // Do not change this code. Put cleanup code in Dispose(bool disposing) above.
            this.Dispose(true);
            GC.SuppressFinalize(this);
        }

        /// <summary>
        /// Dispose.
        /// </summary>
        /// <param name="disposing">Whether it is called from <see cref="Dispose()"/>.</param>
        protected virtual void Dispose(bool disposing)
        {
            if (!this.disposedValue)
            {
                if (disposing)
                {
                    // dispose managed state (managed objects).
                }

                // free unmanaged resources (unmanaged objects) and override a finalizer below.
                // set large fields to null.
                this.Release();
                this.disposedValue = true;
            }
        }

        /// <summary>
        /// Gets the COM method delegate.
        /// </summary>
        /// <typeparam name="TDelegate">The delegate type.</typeparam>
        /// <returns>The delegate.</returns>
        protected TDelegate GetMethod<TDelegate>()
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
    }
}
