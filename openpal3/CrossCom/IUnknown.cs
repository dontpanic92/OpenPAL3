// <copyright file="IUnknown.cs">
// Copyright (c) Shengqiu Li and OpenPAL3 Developers. All rights reserved.
// Licensed under the GPLv3 license. See LICENSE file in the project root for full license information.
// </copyright>

namespace CrossCom
{
    using System;
    using System.Diagnostics.CodeAnalysis;
    using CrossCom.Attributes;

    /// <summary>
    /// Represents an unknown COM object.
    /// </summary>
    [CrossComInterfaceImport("00000001-0000-0000-C000-000000000046", typeof(IUnknownImportedObject))]
    [SuppressMessage("StyleCop.CSharp.NamingRules", "SA1300:ElementMustBeginWithUpperCaseLetter", Justification = "Delegates represent the raw COM types.")]
    [SuppressMessage("StyleCop.CSharp.DocumentationRules", "SA1600:ElementsMustBeDocumented", Justification = "Delegates represent the raw COM types.")]
    public interface IUnknown
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
        ComObject<TInterface>? QueryInterface<TInterface>()
            where TInterface : class, IUnknown;

        /// <summary>
        /// Increase reference count.
        /// </summary>
        /// <returns>Informational reference count.</returns>
        long AddRef();

        /// <summary>
        /// Decrease reference count.
        /// </summary>
        /// <returns>Informational reference count. When it returns 0, the object should be destroyed.</returns>
        long Release();

        /// <summary>
        /// Get the raw COM ptr.
        /// </summary>
        /// <returns>Raw COM ptr.</returns>
        IntPtr GetComPtr();
    }
}
