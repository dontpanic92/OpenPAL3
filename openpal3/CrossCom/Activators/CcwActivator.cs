// <copyright file="CcwActivator.cs">
// Copyright (c) Shengqiu Li and OpenPAL3 Developers. All rights reserved.
// Licensed under the GPLv3 license. See LICENSE file in the project root for full license information.
// </copyright>

namespace CrossCom.Activators
{
    using System;
    using System.Collections.Concurrent;
    using System.Linq.Expressions;
    using CrossCom.Metadata;

    /// <summary>
    /// Create an instance of the ccw type.
    /// TODO: LinqExpression is not AOT friendly.
    /// </summary>
    internal class CcwActivator
    {
        private static readonly ConcurrentDictionary<Type, Activator> Activators = new ConcurrentDictionary<Type, Activator>();

        static CcwActivator()
        {
        }

        private delegate object Activator();

        /// <summary>
        /// Create an instance to wrap the COM ptr.
        /// </summary>
        /// <param name="interfaceType">The interface type.</param>
        /// <returns>The created instance.</returns>
        public static IUnknownCcw CreateInstance(Type interfaceType)
        {
            var activator = Activators.GetOrAdd(interfaceType, (ty) =>
            {
                var ctor = InterfaceMetadata.GetValue(ty).CcwType.GetConstructor(Type.EmptyTypes);
                return (Activator)Expression.Lambda(typeof(Activator), Expression.New(ctor)).Compile();
            });

            return (activator() as IUnknownCcw) !;
        }
    }
}
