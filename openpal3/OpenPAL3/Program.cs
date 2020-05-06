using CrossCom;
using OpenGB.Runtime;
using System;
using System.Runtime.InteropServices;

namespace OpenPAL3
{
    class Program
    {


        [StructLayout(LayoutKind.Sequential)]
        struct ApplicationExtensionVirtualTable
        {
            public IntPtr OnInitialized;
            public IntPtr OnUpdating;
        }

        [StructLayout(LayoutKind.Sequential)]
        public struct ApplicationExtensionExportedRawObject
        {
            public IntPtr VirtualTable;
        }

        public class ApplicationExtensionExportedObject
        {
            private GCHandle rawObject;
            private GCHandle table;
            private IApplicationExtension implementation;

            public ApplicationExtensionExportedObject(IApplicationExtension implementation)
            {
                var rawObject = new ApplicationExtensionExportedRawObject();
                var table = new ApplicationExtensionVirtualTable();
                table.OnInitialized = Marshal.GetFunctionPointerForDelegate<IApplicationExtension._OnInitialized>(this.OnInitializedStub);

                this.table = GCHandle.Alloc(table, GCHandleType.Pinned);

                rawObject.VirtualTable = this.table.AddrOfPinnedObject();
                this.rawObject = GCHandle.Alloc(rawObject, GCHandleType.Pinned);
            }

            public void OnInitializedStub(IntPtr ptr, IntPtr app)
            {
                Console.WriteLine("InOninitialzedStub!");
                this.implementation.OnInitialized(null);
            }

        }

        public class ApplicationExtension : IApplicationExtension
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
        }

        static void Main(string[] args)
        {
            using var factory = ClassFactory<Factory>.Factory.CreateInstance<IFactory>();
            var value = factory.Echo(10);
            Console.WriteLine($"Echo: {value}");

            var result = factory.LoadOpengbConfig("openpal3", "OpenPAL3", out var config);
            var result2 = factory.CreateDefaultApplication(config, "OpenPAL3", out var app);

            app.Initialize();
            app.Run();
        }
    }
}
