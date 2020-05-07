// <copyright file="ImportedInterfaceMetadata.cs">
// Copyright (c) Shengqiu Li and OpenPAL3 Developers. All rights reserved.
// Licensed under the GPLv3 license. See LICENSE file in the project root for full license information.
// </copyright>

namespace CrossCom.Metadata
{
    using System;
    using System.Collections.Concurrent;
    using System.Diagnostics.CodeAnalysis;
    using System.Linq;
    using System.Reflection;
    using CrossCom.Attributes;

    /// <summary>
    /// The metadata for imported interfaces.
    /// </summary>
    internal class ImportedInterfaceMetadata
    {
        private static readonly ConcurrentDictionary<Type, ImportedInterfaceMetadata> Cache = new ConcurrentDictionary<Type, ImportedInterfaceMetadata>();

        /// <summary>
        /// Initializes a new instance of the <see cref="ImportedInterfaceMetadata"/> class.
        /// </summary>
        /// <param name="type">The interface type.</param>
        public ImportedInterfaceMetadata(Type type)
        {
            var attribute = type.GetCustomAttribute(typeof(CrossComInterfaceImport), false) as CrossComInterfaceImport
                ?? throw new InvalidOperationException($"Type {type} doesn't have {nameof(CrossComInterfaceImport)} attribute.");

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
                .Where(attr => attr.Attr != null)
                .OrderBy(attr => attr.Attr!.Order)
                .ToList();

            for (int i = 0; i < attributes.Count; i++)
            {
                VirtualMethodMetadata.AddValue(attributes[i].Delegate, i + parentVirtualTableSize);
            }

            this.VirtualTableSize = parentVirtualTableSize + attributes.Count;
        }

        /// <summary>
        /// Gets the interface id.
        /// </summary>
        public Guid Guid { get; }

        /// <summary>
        /// Gets the corresponding implemenation type.
        /// </summary>
        public Type Implementation { get; }

        /// <summary>
        /// Gets the virtual table size of this interface.
        /// </summary>
        public int VirtualTableSize { get; }

        /// <summary>
        /// Gets the metadata for the given interface type.
        /// </summary>
        /// <param name="type">The interface type.</param>
        /// <returns>Its metadata.</returns>
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

    /// <summary>
    /// A convenient class for retrieving the metadata.
    /// </summary>
    /// <typeparam name="T">The interface type.</typeparam>
    [SuppressMessage("StyleCop.CSharp.MaintainabilityRules", "SA1402:FileMayOnlyContainASingleType", Justification = "This is the generic version.")]
    internal class ImportedInterfaceMetadata<T>
    {
        static ImportedInterfaceMetadata()
        {
            Value = ImportedInterfaceMetadata.GetValue(typeof(T));
        }

        /// <summary>
        /// Gets the cached value.
        /// </summary>
        public static ImportedInterfaceMetadata Value { get; }
    }
}
