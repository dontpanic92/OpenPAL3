// <copyright file="RcwActivator.cs">
// Copyright (c) Shengqiu Li and OpenPAL3 Developers. All rights reserved.
// Licensed under the GPLv3 license. See LICENSE file in the project root for full license information.
// </copyright>

namespace CrossCom.Activators
{
    using System;
    using System.Linq.Expressions;
    using CrossCom.Metadata;

    /// <summary>
    /// Create an instance of the rcw type.
    /// TODO: LinqExpression is not AOT friendly.
    /// </summary>
    /// <typeparam name="TInterface">The interface type.</typeparam>
    internal class RcwActivator<TInterface>
        where TInterface : class, IUnknown
    {
        private static readonly Activator Constructor;

        static RcwActivator()
        {
            var ctor = InterfaceMetadata<TInterface>.Value.RcwType.GetConstructor(new Type[] { typeof(IntPtr) });
            var param = Expression.Parameter(typeof(IntPtr), "ptr");
            Constructor = (Activator)Expression.Lambda(typeof(Activator), Expression.New(ctor, param), param).Compile();
        }

        private delegate object Activator(IntPtr ptr);

        /// <summary>
        /// Create an instance to wrap the COM ptr.
        /// </summary>
        /// <param name="ptr">The COM ptr.</param>
        /// <returns>The created instance.</returns>
        public static TInterface CreateInstance(IntPtr ptr)
        {
            return (Constructor(ptr) as TInterface) !;
        }
    }
}
