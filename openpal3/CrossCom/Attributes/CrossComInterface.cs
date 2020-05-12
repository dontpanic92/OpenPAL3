// <copyright file="CrossComInterface.cs">
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
    public class CrossComInterface : Attribute
    {
        /// <summary>
        /// Initializes a new instance of the <see cref="CrossComInterface"/> class.
        /// </summary>
        /// <param name="rcwType">The rcw type for this interface.</param>
        /// <param name="ccwType">The ccs type for this interface.</param>
        public CrossComInterface(Type rcwType, Type ccwType)
        {
            this.RcwType = rcwType;
            this.CcwType = ccwType;
        }

        /// <summary>
        /// Gets the corresponding rcw type.
        /// </summary>
        public Type RcwType { get; }

        /// <summary>
        /// Gets the corresponding ccw type.
        /// </summary>
        public Type CcwType { get; }
    }
}
