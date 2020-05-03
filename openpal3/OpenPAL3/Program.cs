using CrossCom;
using OpenGB.Runtime;
using System;

namespace OpenPAL3
{
    class Program
    {
        static void Main(string[] args)
        {
            using var factory = ClassFactory<Factory>.Factory.CreateInstance<IFactory>();
            var value = factory.Echo(10);
            Console.WriteLine($"Hello World! {value}");

            var result = factory.LoadOpengbConfig("openpal3", "OpenPAL3", out var config);
            Console.WriteLine($"Result: {result}");

            var result2 = factory.CreateApplication(config, "OpenPAL3", out var app);
            Console.WriteLine($"Result2: {result2}");

            app.Initialize();
            app.Run();
        }
    }
}
