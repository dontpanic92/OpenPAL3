// <copyright file="IUnknownImportedObject.cs">
// Copyright (c) Shengqiu Li and OpenPAL3 Developers. All rights reserved.
// Licensed under the GPLv3 license. See LICENSE file in the project root for full license information.
// </copyright>

namespace CrossCom
{
    using System;
    using CrossCom.Metadata;

    /// <summary>
    /// The implementation for <see cref="IUnknown"/>.
    /// </summary>
    public class IUnknownImportedObject : ImportedObject, IUnknown
    {
        /// <summary>
        /// Initializes a new instance of the <see cref="IUnknownImportedObject"/> class.
        /// </summary>
        /// <param name="ptr">The raw COM ptr.</param>
        public IUnknownImportedObject(IntPtr ptr)
            : base(ptr)
        {
        }

        /// <inheritdoc/>
        public long AddRef()
        {
            return this.GetMethod<IUnknown._AddRef>()(this.GetComPtr());
        }

        /// <inheritdoc/>
        public ComObject<TInterface>? QueryInterface<TInterface>()
            where TInterface : class, IUnknown
        {
            var result = this.GetMethod<IUnknown._QueryInterface>()(this.GetComPtr(), ImportedInterfaceMetadata<TInterface>.Value.Guid, out var ptr);
            if (result == 0)
            {
                var obj = new ComObject<TInterface>(ObjectActivator<TInterface>.CreateInstance(ptr));
                return obj;
            }

            return null;
        }

        /// <inheritdoc/>
        public long Release()
        {
            return this.GetMethod<IUnknown._Release>()(this.GetComPtr());
        }
    }
}
