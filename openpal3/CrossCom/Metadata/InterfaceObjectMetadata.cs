// <copyright file="InterfaceObjectMetadata.cs">
// Copyright (c) Shengqiu Li and OpenPAL3 Developers. All rights reserved.
// Licensed under the GPLv3 license. See LICENSE file in the project root for full license information.
// </copyright>

namespace CrossCom.Metadata
{
    using System;
    using System.Collections.Concurrent;
    using System.Diagnostics.CodeAnalysis;
    using System.Linq;

    /// <summary>
    /// The metadata for the implementation of imported interfaces.
    /// </summary>
    internal class InterfaceObjectMetadata
    {
        private static readonly ConcurrentDictionary<Type, InterfaceObjectMetadata> Cache = new ConcurrentDictionary<Type, InterfaceObjectMetadata>();

        /// <summary>
        /// Initializes a new instance of the <see cref="InterfaceObjectMetadata"/> class.
        /// </summary>
        /// <param name="type">The implementation type.</param>
        public InterfaceObjectMetadata(Type type)
        {
            var parent = type.GetInterfaces().OrderBy(t => t.GetInterfaces().Length).LastOrDefault();
            this.VirtualTablesize = ImportedInterfaceMetadata.GetValue(parent).VirtualTableSize;
        }

        /// <summary>
        /// Gets the vtable size of this object.
        /// </summary>
        public int VirtualTablesize { get; }

        /// <summary>
        /// Gets the metadata for the given implementation type.
        /// </summary>
        /// <param name="type">The implementation type.</param>
        /// <returns>Its metadata.</returns>
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

    /// <summary>
    /// A convenient class for retrieving the metadata.
    /// </summary>
    /// <typeparam name="T">The interface implementation type.</typeparam>
    [SuppressMessage("StyleCop.CSharp.MaintainabilityRules", "SA1402:FileMayOnlyContainASingleType", Justification = "This is the generic version.")]
    internal class InterfaceObjectMetadata<T>
    {
        static InterfaceObjectMetadata()
        {
            Value = InterfaceObjectMetadata.GetValue(typeof(T));
        }

        /// <summary>
        /// Gets the cached metadata.
        /// </summary>
        public static InterfaceObjectMetadata Value { get; }
    }
}
