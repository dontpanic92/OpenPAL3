// <copyright file="CrossComMethod.cs">
// Copyright (c) Shengqiu Li and OpenPAL3 Developers. All rights reserved.
// Licensed under the GPLv3 license. See LICENSE file in the project root for full license information.
// </copyright>

namespace CrossCom.Attributes
{
    using System;
    using System.Runtime.CompilerServices;

    /// <summary>
    /// The attribute to indicate the delegate represents a COM method signature.
    /// </summary>
    [AttributeUsage(AttributeTargets.Delegate, Inherited = false, AllowMultiple = false)]
    public class CrossComMethod : Attribute
    {
        /// <summary>
        /// Initializes a new instance of the <see cref="CrossComMethod"/> class.
        /// </summary>
        /// <param name="order">The method line number in the source file.</param>
        public CrossComMethod([CallerLineNumber] int order = 0)
        {
            this.Order = order;
        }

        /// <summary>
        /// Gets the delegate's relative order.
        /// </summary>
        public int Order { get; }
    }
}
