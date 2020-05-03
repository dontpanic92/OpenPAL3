namespace CrossCom.CodeGen
{
    using System;
    using System.IO;

    class Program
    {
        public static void Main(string[] args)
        {
            if (args.Length < 4)
            {
                throw new ArgumentException("Usage: CrossCom.CodeGen.exe ProjectPath IdlJsonFile OutputFolder ConfigFile");
            }

            Console.WriteLine(args[0]);
            Console.WriteLine(args[1]);
            Console.WriteLine(args[2]);
            Console.WriteLine(args[3]);
            new CodeGenerator(Path.Combine(args[0], args[1]), Path.Combine(args[0], args[2]), Path.Combine(args[0], args[3])).Generate();
        }
    }
}
