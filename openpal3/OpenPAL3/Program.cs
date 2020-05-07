using CrossCom;
using OpenGB.Runtime;
using System;
using System.Runtime.InteropServices;

namespace OpenPAL3
{
    class Program
    {


        [StructLayout(LayoutKind.Sequential)]
        struct ApplicationExtensionExportedVirtualTable
        {
            public readonly IntPtr OnInitialized;
            public readonly IntPtr OnUpdating;

            private readonly IApplicationExtension impl;
            public ApplicationExtensionExportedVirtualTable(IApplicationExtension impl)
            {
                this.impl = impl;
                this.OnInitialized = IntPtr.Zero;
                this.OnUpdating = IntPtr.Zero;

                this.OnInitialized = Marshal.GetFunctionPointerForDelegate<IApplicationExtension._OnInitialized>(this.OnInitializedStub);
                this.OnUpdating = Marshal.GetFunctionPointerForDelegate<IApplicationExtension._OnUpdating>(this.OnUpdatingStub);
            }

            private void OnInitializedStub(IntPtr ptr, IntPtr app)
            {
                impl.OnInitialized(null);
            }

            private void OnUpdatingStub(IntPtr ptr, IntPtr app, float deltaSec)
            {
                impl.OnUpdating(null, deltaSec);
            }
        }

        [StructLayout(LayoutKind.Sequential)]
        public struct ExportedRawObject
        {
            public IntPtr VirtualTable;
        }

        class ApplicationExtensionExportedObject
        {
            private GCHandle rawObject;
            private GCHandle table;

            public ApplicationExtensionExportedObject(IApplicationExtension impl)
            {
                var table = new ApplicationExtensionExportedVirtualTable(impl);
                this.table = GCHandle.Alloc(table, GCHandleType.Pinned);

                var rawObject = new ExportedRawObject
                {
                    VirtualTable = this.table.AddrOfPinnedObject(),
                };
                this.rawObject = GCHandle.Alloc(rawObject, GCHandleType.Pinned);
            }
        }

        public class ApplicationExtension : IApplicationExtension
        {
            private IApplicationExtension implementation;

            public ApplicationExtension()
            {
            }

            public long AddRef()
            {
                throw new NotImplementedException();
            }

            public IntPtr GetComPtr()
            {
                throw new NotImplementedException();
            }

            public void OnInitialized(ComObject<IApplication> app)
            {
                throw new NotImplementedException();
            }


            public void OnUpdating(ComObject<IApplication> app, float delta_sec)
            {
                throw new NotImplementedException();
            }

            public long Release()
            {
                throw new NotImplementedException();
            }

            ComObject<TInterface> IUnknown.QueryInterface<TInterface>()
            {
                throw new NotImplementedException();
            }
        }

        /*public class ApplicationExtension : IApplicationExtension
        {
            

            public IntPtr GetComPtr()
            {
                throw new NotImplementedException();
            }

            public void OnInitialized(IApplication app)
            {
                throw new NotImplementedException();
            }

            public void OnUpdating(IApplication app, float delta_sec)
            {
                throw new NotImplementedException();
            }

            TInterface IUnknown.QueryInterface<TInterface>()
            {
                throw new NotImplementedException();
            }
        }*/

        static void Main(string[] args)
        {
            using var factory = ClassFactory<Factory>.Factory.CreateInstance<IFactory>();
            var value = factory.Get().Echo(10);
            Console.WriteLine($"Echo: {value}");

            var result = factory.Get().LoadOpengbConfig("openpal3", "OpenPAL3", out var config);
            var result2 = factory.Get().CreateDefaultApplication(config, "OpenPAL3", out var app);

            app.Get().Initialize();
            app.Get().Run();
        }
    }
}
