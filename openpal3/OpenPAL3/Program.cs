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

            var ptr = factory.LoadOpengbConfig("openpal3", "OpenPAL3");
            Console.WriteLine($"Ptr: {ptr}");
        }
    }
}
