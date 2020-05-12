using CrossCom;
using OpenGB.Runtime;
using System;
using System.Runtime.InteropServices;

namespace OpenPAL3
{
    class Program
    {
        public class ApplicationExtension : Unknown, IApplicationExtension
        {
            private IApplication app;

            public ApplicationExtension()
            {
            }

            public void OnInitialized(IApplication app)
            {
                Console.WriteLine("OnInitialized");
                this.app = app;
            }

            public void OnUpdating(IApplication app, float delta_sec)
            {
                Console.WriteLine("OnUpdating");
            }
        }

        static void Main(string[] args)
        {
            using var factory = ClassFactory<Factory>.Factory.CreateInstance<IFactory>();
            var value = factory.Echo(10);
            Console.WriteLine($"Echo: {value}");

            /*var appext = new ApplicationExtension();
            var result = factory.CreateApplication(appext, out var app);
            app.Initialize();
            app.Run();*/

            var result = factory.LoadOpengbConfig("openpal3", "OpenPAL3", out var config);
            var result2 = factory.CreateDefaultApplication(config, "OpenPAL3", out var app);

            app.Initialize();
            app.Run();
        }
    }
}
