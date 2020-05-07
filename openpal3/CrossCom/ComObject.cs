// <copyright file="ComObject.cs">
// Copyright (c) Shengqiu Li and OpenPAL3 Developers. All rights reserved.
// Licensed under the GPLv3 license. See LICENSE file in the project root for full license information.
// </copyright>

namespace CrossCom
{
    using System;
    using Common;

    /// <summary>
    /// The COM object wrapper to handle reference counting.
    /// </summary>
    /// <typeparam name="TInterface">The COM interface.</typeparam>
    public sealed class ComObject<TInterface> : IDisposable
        where TInterface : class, IUnknown
    {
        private readonly TInterface comObject;

        /// <summary>
        /// Initializes a new instance of the <see cref="ComObject{TInterface}"/> class.
        /// </summary>
        /// <param name="comObject">The wrapped COM object.</param>
        public ComObject(TInterface comObject)
        {
            Contract.Require(comObject, nameof(comObject));

            this.comObject = comObject;
            this.comObject.AddRef();
        }

        /// <summary>
        /// Finalizes an instance of the <see cref="ComObject{TInterface}"/> class.
        /// </summary>
        ~ComObject()
        {
            this.comObject.Release();
        }

        /// <summary>
        /// Gets the COM object.
        /// </summary>
        /// <returns>The COM object.</returns>
        public TInterface Get()
        {
            return this.comObject;
        }

        /// <summary>
        /// Get the raw COM ptr.
        /// </summary>
        /// <returns>The raw COM ptr.</returns>
        public IntPtr GetComPtr()
        {
            return this.comObject.GetComPtr();
        }

        /// <inheritdoc/>
        public void Dispose()
        {
            this.comObject.Release();
            GC.SuppressFinalize(this);
        }
    }
}
