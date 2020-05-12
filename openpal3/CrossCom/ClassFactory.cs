// <copyright file="ClassFactory.cs">
// Copyright (c) Shengqiu Li and OpenPAL3 Developers. All rights reserved.
// Licensed under the GPLv3 license. See LICENSE file in the project root for full license information.
// </copyright>

namespace CrossCom
{
    using System;
    using System.Diagnostics.CodeAnalysis;
    using CrossCom.Activators;
    using CrossCom.Metadata;

    /// <summary>
    /// A utility class to retrieve class factories for the given COM class type.
    /// </summary>
    /// <typeparam name="TClass">The class type.</typeparam>
    [SuppressMessage("StyleCop.CSharp.MaintainabilityRules", "SA1402:FileMayOnlyContainASingleType", Justification = "This is the generic version.")]
    public class ClassFactory<TClass>
    {
        static ClassFactory()
        {
            NativeMethods.DllGetClassObject(typeof(TClass).GUID, typeof(IClassFactory).GUID, out var ptr);
            Factory = new ClassFactory(ptr);
        }

        /// <summary>
        /// Gets the class factory.
        /// </summary>
        public static IClassFactory Factory { get; }
    }

    /// <summary>
    /// The class factory for creating COM objects.
    /// </summary>
    internal class ClassFactory : IUnknownRcw, IClassFactory
    {
        /// <summary>
        /// Initializes a new instance of the <see cref="ClassFactory"/> class.
        /// </summary>
        /// <param name="ptr">COM object ptr.</param>
        public ClassFactory(IntPtr ptr)
            : base(ptr)
        {
        }

        /// <summary>
        /// Create an instance as the given type.
        /// </summary>
        /// <typeparam name="TInterface">The object's interface.</typeparam>
        /// <returns>The created instance.</returns>
        public TInterface CreateInstance<TInterface>()
            where TInterface : class, IUnknown
        {
            this.GetMethod<IClassFactory._CreateInstance>()(this.GetComPtr(typeof(TInterface)), IntPtr.Zero, typeof(TInterface).GUID, out var ptr);
            return RcwActivator<TInterface>.CreateInstance(ptr);
        }
    }
}
