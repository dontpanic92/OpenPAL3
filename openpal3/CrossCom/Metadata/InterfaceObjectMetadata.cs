using System;
using System.Collections.Concurrent;
using System.Collections.Generic;
using System.Linq;
using System.Reflection;
using System.Text;

namespace CrossCom.Metadata
{
    internal class InterfaceObjectMetadata
    {
        private static readonly ConcurrentDictionary<Type, InterfaceObjectMetadata> Cache = new ConcurrentDictionary<Type, InterfaceObjectMetadata>();

        public InterfaceObjectMetadata(Type type)
        {
            var parent = type.GetInterfaces().OrderBy(t => t.GetInterfaces().Length).LastOrDefault();
            this.VirtualTablesize = ImportedInterfaceMetadata.GetValue(parent).VirtualTableSize;
        }

        public int VirtualTablesize { get; }

        public static InterfaceObjectMetadata GetValue(Type type)
        {
            if (Cache.TryGetValue(type, out var value))
            {
                return value;
            }

            value = new InterfaceObjectMetadata(type);
            Cache.TryAdd(type, value);
            return value;
        }
    }

    internal class InterfaceObjectMetadata<T>
    {
        static InterfaceObjectMetadata()
        {
            Value = InterfaceObjectMetadata.GetValue(typeof(T));
        }

        public static InterfaceObjectMetadata Value { get; }
    }
}
