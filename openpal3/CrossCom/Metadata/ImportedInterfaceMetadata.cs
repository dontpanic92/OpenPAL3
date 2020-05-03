using CrossCom.Attributes;
using System;
using System.Collections.Concurrent;
using System.Collections.Generic;
using System.Linq;
using System.Reflection;
using System.Text;

namespace CrossCom.Metadata
{
    internal class ImportedInterfaceMetadata
    {
        private static readonly ConcurrentDictionary<Type, ImportedInterfaceMetadata> Cache = new ConcurrentDictionary<Type, ImportedInterfaceMetadata>();

        public ImportedInterfaceMetadata(Type type)
        {
            var attribute = type.GetCustomAttribute(typeof(CrossComInterfaceImport), false) as CrossComInterfaceImport;
            this.Guid = Guid.Parse(attribute.Guid);
            this.Implementation = attribute.Implementation;

            var parent = type.GetInterfaces().OrderBy(t => t.GetInterfaces().Length).LastOrDefault();
            var parentVirtualTableSize = 0;
            if (type != typeof(IUnknown) && parent != null)
            {
                parentVirtualTableSize = GetValue(parent).VirtualTableSize;
            }

            var delegates = type.GetNestedTypes(BindingFlags.Public | BindingFlags.NonPublic).Where(t => typeof(Delegate).IsAssignableFrom(t));
            var attributes = delegates.Select(t => new { Delegate = t, Attr = t.GetCustomAttribute(typeof(CrossComMethod), false) as CrossComMethod })
                .Where(attr => attr.Attr!= null)
                .OrderBy(attr => attr.Attr.Order)
                .ToList();

            for (int i = 0; i < attributes.Count; i++)
            {
                VirtualMethodMetadata.AddValue(attributes[i].Delegate, i + parentVirtualTableSize);
            }

            this.VirtualTableSize = parentVirtualTableSize + attributes.Count;
        }

        public Guid Guid { get; }

        public Type Implementation { get; }

        public int VirtualTableSize { get; }

        public static ImportedInterfaceMetadata GetValue(Type type)
        {
            if (Cache.TryGetValue(type, out var value))
            {
                return value;
            }

            value = new ImportedInterfaceMetadata(type);
            Cache.TryAdd(type, value);
            return value;
        }

    }

    internal class ImportedInterfaceMetadata<T>
    {
        static ImportedInterfaceMetadata()
        {
            Value = ImportedInterfaceMetadata.GetValue(typeof(T));
        }

        public static ImportedInterfaceMetadata Value { get; }
    }
}
