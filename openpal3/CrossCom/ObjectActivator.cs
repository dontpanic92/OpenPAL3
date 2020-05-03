using CrossCom.Attributes;
using CrossCom.Metadata;
using System;
using System.Collections.Generic;
using System.Linq.Expressions;
using System.Text;

namespace CrossCom
{
    class ObjectActivator<TInterface>
        where TInterface: class, IUnknown
    {
        delegate object Activator(IntPtr ptr);

        private static readonly Activator Constructor;

        static ObjectActivator()
        {
            var ctor = ImportedInterfaceMetadata<TInterface>.Value.Implementation.GetConstructor(new Type[] { typeof(IntPtr) });
            var param = Expression.Parameter(typeof(IntPtr), "ptr");
            Constructor = (Activator)Expression.Lambda(typeof(Activator), Expression.New(ctor, param), param).Compile();
        }

        public static TInterface CreateInstance(IntPtr ptr)
        {
            return Constructor(ptr) as TInterface;
        }
    }
}
