// <copyright file="ImportedObjectMetadata.cs">
// Copyright (c) Shengqiu Li and OpenPAL3 Developers. All rights reserved.
// Licensed under the GPLv3 license. See LICENSE file in the project root for full license information.
// </copyright>

namespace CrossCom.Metadata
{
    using System;
    using System.Collections.Concurrent;
    using System.Diagnostics.CodeAnalysis;
    using System.Reflection;
    using CrossCom.Attributes;

    /// <summary>
    /// The metadata for imported objects.
    /// </summary>
    internal class ImportedObjectMetadata
    {
        private static readonly ConcurrentDictionary<Type, ImportedObjectMetadata> Cache = new ConcurrentDictionary<Type, ImportedObjectMetadata>();

        /// <summary>
        /// Initializes a new instance of the <see cref="ImportedObjectMetadata"/> class.
        /// </summary>
        /// <param name="type">The object type.</param>
        public ImportedObjectMetadata(Type type)
        {
            var attribute = type.GetCustomAttribute(typeof(CrossComObjectImport), false) as CrossComObjectImport
                ?? throw new InvalidOperationException($"Type {type} doesn't have {nameof(CrossComObjectImport)} attribute.");

            this.Guid = Guid.Parse(attribute.Guid);
        }

        /// <summary>
        /// Gets the class id.
        /// </summary>
        public Guid Guid { get; }

        /// <summary>
        /// Gets the metadata for the given object type.
        /// </summary>
        /// <param name="type">The object type.</param>
        /// <returns>Its metadata.</returns>
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

    /// <summary>
    /// A convenient class for retrieving the metadata.
    /// </summary>
    /// <typeparam name="T">The object type.</typeparam>
    [SuppressMessage("StyleCop.CSharp.MaintainabilityRules", "SA1402:FileMayOnlyContainASingleType", Justification = "This is the generic version.")]
    internal class ImportedObjectMetadata<T>
    {
        static ImportedObjectMetadata()
        {
            Value = ImportedObjectMetadata.GetValue(typeof(T));
        }

        /// <summary>
        /// Gets the cached metadata.
        /// </summary>
        public static ImportedObjectMetadata Value { get; }
    }
}
