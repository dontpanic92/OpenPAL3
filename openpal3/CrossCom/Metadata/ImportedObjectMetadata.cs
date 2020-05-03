using CrossCom.Attributes;
using System;
using System.Collections.Concurrent;
using System.Collections.Generic;
using System.Linq;
using System.Reflection;
using System.Text;

namespace CrossCom.Metadata
{

    internal class ImportedObjectMetadata
    {
        private static readonly ConcurrentDictionary<Type, ImportedObjectMetadata> Cache = new ConcurrentDictionary<Type, ImportedObjectMetadata>();

        public ImportedObjectMetadata(Type type)
        {
            var attribute = type.GetCustomAttribute(typeof(CrossComObjectImport), false) as CrossComObjectImport;
            this.Guid = Guid.Parse(attribute.Guid);
        }

        public Guid Guid { get; }

        public static ImportedObjectMetadata GetValue(Type type)
        {
            if (Cache.TryGetValue(type, out var value))
            {
                return value;
            }

            value = new ImportedObjectMetadata(type);
            Cache.TryAdd(type, value);
            return value;
        }
    }

    internal class ImportedObjectMetadata<T>
    {
        static ImportedObjectMetadata()
        {
            Value = ImportedObjectMetadata.GetValue(typeof(T));
        }

        public static ImportedObjectMetadata Value { get; }
    }
}
