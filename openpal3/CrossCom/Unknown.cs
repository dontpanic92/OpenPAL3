// <copyright file="Unknown.cs">
// Copyright (c) Shengqiu Li and OpenPAL3 Developers. All rights reserved.
// Licensed under the GPLv3 license. See LICENSE file in the project root for full license information.
// </copyright>

namespace CrossCom
{
    using System;
    using System.Collections.Concurrent;
    using System.Runtime.InteropServices;
    using System.Threading;
    using CrossCom.Activators;

    /// <summary>
    /// The base for all the exported COM classes.
    /// </summary>
    public class Unknown : IUnknown, IUnknownInternal, IComObject
    {
        private readonly ConcurrentDictionary<Type, IUnknownCcw> wrappers = new ConcurrentDictionary<Type, IUnknownCcw>();
        private long referenceCount = 0;
        private GCHandle? selfHandle;
        private bool disposedValue = false;

        /// <summary>
        /// Gets the object cache.
        /// </summary>
        public static ConcurrentDictionary<IntPtr, Unknown> ObjectCache { get; } = new ConcurrentDictionary<IntPtr, Unknown>();

        /// <inheritdoc/>
        public void Dispose()
        {
            if (!this.disposedValue)
            {
                if (this.referenceCount == 0)
                {
                    this.DisposeInternal();
                }

                this.disposedValue = true;
            }
        }

        /// <inheritdoc/>
        public TInterface? QueryInterface<TInterface>()
            where TInterface : class, IUnknown
        {
            return this as TInterface;
        }

        /// <inheritdoc/>
        public IntPtr GetComPtr(Type interfaceType)
        {
            var wrapper = this.wrappers.GetOrAdd(interfaceType, (ty) =>
            {
                return CcwActivator.CreateInstance(interfaceType);
            });

            this.AddRef();
            var ptr = wrapper.GetComPtr();
            _ = ObjectCache.TryAdd(ptr, this);

            return ptr;
        }

        /// <summary>
        /// Increase the reference count. Should be only called from COM.
        /// </summary>
        /// <returns>The incremented count.</returns>
        public long AddRef()
        {
            long count = Interlocked.Increment(ref this.referenceCount);
            if (count == 1)
            {
                this.selfHandle = GCHandle.Alloc(this);
            }

            return count;
        }

        /// <summary>
        /// Decrease the reference count. Should be only called from COM.
        /// </summary>
        /// <returns>The decreased count.</returns>
        public long Release()
        {
            long count = Interlocked.Decrement(ref this.referenceCount);
            if (count == 0)
            {
                this.selfHandle!.Value.Free();
            }

            return count;
        }

        /// <summary>
        /// Dispose all the resources allocated.
        /// </summary>
        protected virtual void DisposeInternal()
        {
            foreach (var wrapper in this.wrappers.Values)
            {
                _ = ObjectCache.TryRemove(wrapper.GetComPtr(), out var _);
                wrapper.Dispose();
            }
        }
    }
}
