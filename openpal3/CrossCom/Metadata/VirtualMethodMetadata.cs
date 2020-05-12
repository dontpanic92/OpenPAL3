// <copyright file="VirtualMethodMetadata.cs">
// Copyright (c) Shengqiu Li and OpenPAL3 Developers. All rights reserved.
// Licensed under the GPLv3 license. See LICENSE file in the project root for full license information.
// </copyright>

namespace CrossCom.Metadata
{
    using System;
    using System.Collections.Concurrent;
    using System.Collections.Generic;
    using System.Diagnostics.CodeAnalysis;

    /// <summary>
    /// The metadata for the COM method delegate types.
    /// </summary>
    internal class VirtualMethodMetadata
    {
        private static readonly ConcurrentDictionary<Type, VirtualMethodMetadata> Cache = new ConcurrentDictionary<Type, VirtualMethodMetadata>();

        /// <summary>
        /// Initializes a new instance of the <see cref="VirtualMethodMetadata"/> class.
        /// </summary>
        /// <param name="index">The method index in the vtable.</param>
        public VirtualMethodMetadata(int index)
        {
            this.Index = index;
        }

        /// <summary>
        /// Gets the method index.
        /// </summary>
        public int Index { get; }

        /// <summary>
        /// Gets the metadata for the given delegate.
        /// </summary>
        /// <param name="type">The delegate type.</param>
        /// <returns>Its metadata.</returns>
        public static VirtualMethodMetadata GetValue(Type type)
        {
            if (Cache.TryGetValue(type, out var value))
            {
                return value;
            }

            throw new KeyNotFoundException($"Method attribute not added for type {type}");
        }

        /// <summary>
        /// Add new metadata for the delegate.
        /// </summary>
        /// <param name="type">The delegate type.</param>
        /// <param name="index">The method index.</param>
        internal static void AddValue(Type type, int index)
        {
            Cache.TryAdd(type, new VirtualMethodMetadata(index));
        }
    }

    /// <summary>
    /// A convenient class for retrieving the metadata.
    /// </summary>
    /// <typeparam name="T">The delegate type.</typeparam>
    [SuppressMessage("StyleCop.CSharp.MaintainabilityRules", "SA1402:FileMayOnlyContainASingleType", Justification = "This is the generic version.")]
    internal class VirtualMethodMetadata<T>
    {
        static VirtualMethodMetadata()
        {
            Value = VirtualMethodMetadata.GetValue(typeof(T));
        }

        /// <summary>
        /// Gets the cached metadata.
        /// </summary>
        public static VirtualMethodMetadata Value { get; }
    }
}
