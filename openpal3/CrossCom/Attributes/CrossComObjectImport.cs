// <copyright file="CrossComObjectImport.cs">
// Copyright (c) Shengqiu Li and OpenPAL3 Developers. All rights reserved.
// Licensed under the GPLv3 license. See LICENSE file in the project root for full license information.
// </copyright>

namespace CrossCom.Attributes
{
    using System;

    /// <summary>
    /// The attribute to indicate a class is imported from external COM dlls.
    /// </summary>
    public class CrossComObjectImport : Attribute
    {
        /// <summary>
        /// Initializes a new instance of the <see cref="CrossComObjectImport"/> class.
        /// </summary>
        /// <param name="guid">The class id.</param>
        public CrossComObjectImport(string guid)
        {
            this.Guid = guid;
        }

        /// <summary>
        /// Gets the class id.
        /// </summary>
        public string Guid { get; }
    }
}
