using System;
using System.Collections.Concurrent;
using System.Collections.Generic;
using System.Reflection;
using System.Text;

namespace CrossCom.Metadata
{
    internal class VirtualMethodMetadata
    {
        private static readonly ConcurrentDictionary<Type, VirtualMethodMetadata> Cache = new ConcurrentDictionary<Type, VirtualMethodMetadata>();

        public VirtualMethodMetadata(int index)
        {
            this.Index = index;
        }

        public int Index { get; }

        public static VirtualMethodMetadata GetValue(Type type)
        {
            if (Cache.TryGetValue(type, out var value))
            {
                return value;
            }

            throw new KeyNotFoundException($"Method attribute not added for type {type}");
        }

        internal static void AddValue(Type type, int index)
        {
            Cache.TryAdd(type, new VirtualMethodMetadata(index));
        }
    }

    internal class VirtualMethodMetadata<T>
    {
        static VirtualMethodMetadata()
        {
            Value = VirtualMethodMetadata.GetValue(typeof(T));
        }

        public static VirtualMethodMetadata Value { get; }
    }
}
