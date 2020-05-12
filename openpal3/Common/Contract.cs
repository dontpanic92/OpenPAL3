// <copyright file="Contract.cs">
// Copyright (c) Shengqiu Li and OpenPAL3 Developers. All rights reserved.
// Licensed under the GPLv3 license. See LICENSE file in the project root for full license information.
// </copyright>

namespace Common
{
    using System;

    /// <summary>
    /// Utility class for checking parameters and return values.
    /// </summary>
    internal static class Contract
    {
        /// <summary>
        /// Validate whether a parameter is null.
        /// </summary>
        /// <param name="obj">The parameter value.</param>
        /// <param name="name">The parameter name.</param>
        public static void Require(object obj, string name)
        {
            if (obj == null)
            {
                throw new ArgumentNullException(name);
            }
        }
    }
}
