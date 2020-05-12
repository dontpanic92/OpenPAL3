// <copyright file="IUnknown.cs">
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
    /// Represents an unknown COM object.
    /// </summary>
    [Guid("00000001-0000-0000-C000-000000000046")]
    [CrossComInterface(typeof(IUnknownRcw), typeof(IUnknownCcw))]
    [SuppressMessage("StyleCop.CSharp.NamingRules", "SA1300:ElementMustBeginWithUpperCaseLetter", Justification = "Delegates represent the raw COM types.")]
    [SuppressMessage("StyleCop.CSharp.DocumentationRules", "SA1600:ElementsMustBeDocumented", Justification = "Delegates represent the raw COM types.")]
    public interface IUnknown : IDisposable
    {
        [CrossComMethod]
        public delegate long _QueryInterface(IntPtr self, Guid guid, out IntPtr retval);

        [CrossComMethod]
        public delegate long _AddRef(IntPtr self);

        [CrossComMethod]
        public delegate long _Release(IntPtr self);

        /// <summary>
        /// Cast current interface to the given one.
        /// </summary>
        /// <typeparam name="TInterface">The dest interface.</typeparam>
        /// <returns>Casted object.</returns>
        TInterface? QueryInterface<TInterface>()
            where TInterface : class, IUnknown;
    }

    /// <summary>
    /// Internal interface for IUnknown.
    /// </summary>
    internal interface IUnknownInternal
    {
        /// <summary>
        /// Increase reference count.
        /// </summary>
        /// <returns>The incremented count.</returns>
        long AddRef();

        /// <summary>
        /// Decrease reference count.
        /// </summary>
        /// <returns>The decremented count.</returns>
        long Release();
    }
}
