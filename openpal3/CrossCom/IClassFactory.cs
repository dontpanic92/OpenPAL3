// <copyright file="IClassFactory.cs">
// Copyright (c) Shengqiu Li and OpenPAL3 Developers. All rights reserved.
// Licensed under the GPLv3 license. See LICENSE file in the project root for full license information.
// </copyright>

namespace CrossCom
{
    using System;
    using System.Diagnostics.CodeAnalysis;
    using System.Runtime.InteropServices;
    using CrossCom.Attributes;

    /// <summary>
    /// Represents a factory to create instances of a class.
    /// </summary>
    [Guid("00000001-0000-0000-C000-000000000046")]
    [CrossComInterface(typeof(ClassFactory), typeof(ClassFactory))]
    [SuppressMessage("StyleCop.CSharp.NamingRules", "SA1300:ElementMustBeginWithUpperCaseLetter", Justification = "Delegates represent the raw COM types.")]
    [SuppressMessage("StyleCop.CSharp.DocumentationRules", "SA1600:ElementsMustBeDocumented", Justification = "Delegates represent the raw COM types.")]
    public interface IClassFactory : IUnknown
    {
        [CrossComMethod]
        public delegate long _CreateInstance(IntPtr self, IntPtr outer, [MarshalAs(UnmanagedType.LPStruct)] Guid guid, out IntPtr retval);

        [CrossComMethod]
        public delegate long _LockServer(IntPtr self);

        /// <summary>
        /// Create an instance as the given interface.
        /// </summary>
        /// <typeparam name="TInterface">The interface.</typeparam>
        /// <returns>The created instance.</returns>
        TInterface CreateInstance<TInterface>()
            where TInterface : class, IUnknown;
    }
}
