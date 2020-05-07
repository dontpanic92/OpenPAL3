// <copyright file="Unknown.cs">
// Copyright (c) Shengqiu Li and OpenPAL3 Developers. All rights reserved.
// Licensed under the GPLv3 license. See LICENSE file in the project root for full license information.
// </copyright>

namespace CrossCom
{
    using System;
    using System.Threading;

    /// <summary>
    /// The base for exported COM classes.
    /// </summary>
    public abstract class Unknown : IUnknown, IDisposable
    {
        private long referenceCount = 0;

        /// <inheritdoc/>
        public long AddRef()
        {
            return Interlocked.Increment(ref this.referenceCount);
        }

        /// <inheritdoc/>
        public abstract IntPtr GetComPtr();

        /// <inheritdoc/>
        public long Release()
        {
            return Interlocked.Decrement(ref this.referenceCount);
        }

        /// <inheritdoc/>
        ComObject<TInterface>? IUnknown.QueryInterface<TInterface>()
        {
            if (this is TInterface obj)
            {
                return new ComObject<TInterface>(obj);
            }

            return null;
        }

        private bool disposedValue = false; // To detect redundant calls

        protected virtual void Dispose(bool disposing)
        {
            if (!disposedValue)
            {
                if (disposing)
                {
                    // TODO: dispose managed state (managed objects).
                }

                // TODO: free unmanaged resources (unmanaged objects) and override a finalizer below.
                // TODO: set large fields to null.
                this.
                disposedValue = true;
            }
        }

        // TODO: override a finalizer only if Dispose(bool disposing) above has code to free unmanaged resources.
        ~Unknown()
        {
            // Do not change this code. Put cleanup code in Dispose(bool disposing) above.
            Dispose(false);
        }

        // This code added to correctly implement the disposable pattern.
        public void Dispose()
        {
            // Do not change this code. Put cleanup code in Dispose(bool disposing) above.
            Dispose(true);
            GC.SuppressFinalize(this);
        }
    }
}
