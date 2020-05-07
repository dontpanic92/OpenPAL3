// <copyright file="CrossComInterfaceImport.cs">
// Copyright (c) Shengqiu Li and OpenPAL3 Developers. All rights reserved.
// Licensed under the GPLv3 license. See LICENSE file in the project root for full license information.
// </copyright>

namespace CrossCom.Attributes
{
    using System;

    /// <summary>
    /// The attribute to indicate an interface is imported from external COM dlls.
    /// </summary>
    [AttributeUsage(AttributeTargets.Interface, Inherited = false, AllowMultiple = false)]
    public class CrossComInterfaceImport : Attribute
    {
        /// <summary>
        /// Initializes a new instance of the <see cref="CrossComInterfaceImport"/> class.
        /// </summary>
        /// <param name="guid">The interface id.</param>
        /// <param name="implementation">The implementation type for this interface.</param>
        public CrossComInterfaceImport(string guid, Type implementation)
        {
            this.Guid = guid;
            this.Implementation = implementation;
        }

        /// <summary>
        /// Gets the interface id.
        /// </summary>
        public string Guid { get; }

        /// <summary>
        /// Gets the corresponding implementation type.
        /// </summary>
        public Type Implementation { get; }
    }
}
