// <copyright file="IComObject.cs">
// Copyright (c) Shengqiu Li and OpenPAL3 Developers. All rights reserved.
// Licensed under the GPLv3 license. See LICENSE file in the project root for full license information.
// </copyright>

namespace CrossCom
{
    using System;

    /// <summary>
    /// Represent a COM object.
    /// </summary>
    public interface IComObject
    {
        /// <summary>
        /// Get raw COM ptr for the given interface.
        /// </summary>
        /// <param name="interfaceType">The interface type.</param>
        /// <returns>The raw COM ptr.</returns>
        IntPtr GetComPtr(Type interfaceType);
    }
}
