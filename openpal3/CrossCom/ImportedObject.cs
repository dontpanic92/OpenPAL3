using CrossCom.Attributes;
using CrossCom.Metadata;
using System;
using System.Collections.Generic;
using System.Runtime.InteropServices;
using System.Text;

namespace CrossCom
{
    public abstract class ImportedObject : IDisposable
    {
        private readonly IntPtr self;
        private readonly IntPtr vtable;
        private readonly Delegate[] methods;

        public ImportedObject(IntPtr ptr)
        {
            this.self = ptr;
            this.vtable = Marshal.ReadIntPtr(ptr);
            this.methods = new Delegate[InterfaceObjectMetadata.GetValue(this.GetType()).VirtualTablesize];
            this.GetMethod<IUnknown._AddRef>()(this.self);
        }

        public TDelegate GetMethod<TDelegate>()
            where TDelegate: Delegate
        {
            var index = VirtualMethodMetadata<TDelegate>.Value.Index;
            if (this.methods[index] == null)
            {
                var method = new IntPtr(this.vtable.ToInt64() + index * IntPtr.Size);
                this.methods[index] = Marshal.GetDelegateForFunctionPointer<TDelegate>(Marshal.ReadIntPtr(method));
            }

            return (TDelegate)this.methods[index];
        }

        public IntPtr GetComPtr()
        {
            return this.self;
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

                this.GetMethod<IUnknown._Release>()(this.self);
                disposedValue = true;
            }
        }

        ~ImportedObject()
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
