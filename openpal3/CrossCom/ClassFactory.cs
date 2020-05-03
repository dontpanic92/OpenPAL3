using CrossCom.Attributes;
using CrossCom.Metadata;
using System;
using System.Collections.Generic;
using System.Text;

namespace CrossCom
{
    public class ClassFactory<TClass>
    {
        static ClassFactory()
        {
            NativeMethods.DllGetClassObject(ImportedObjectMetadata<TClass>.Value.Guid, ImportedInterfaceMetadata<IClassFactory>.Value.Guid, out var ptr);
            Factory = new ClassFactory(ptr);
        }

        public static IClassFactory Factory { get; }
    }

    internal class ClassFactory : IUnknownObject, IClassFactory
    {
        public ClassFactory(IntPtr ptr)
            : base(ptr)
        {
        }

        public TInterface CreateInstance<TInterface>()
            where TInterface: class, IUnknown
        {
            this.GetMethod<IClassFactory._CreateInstance>()(this.GetComPtr(), IntPtr.Zero, ImportedInterfaceMetadata<TInterface>.Value.Guid, out var ptr);
            return ObjectActivator<TInterface>.CreateInstance(ptr);
        }
    }
}
