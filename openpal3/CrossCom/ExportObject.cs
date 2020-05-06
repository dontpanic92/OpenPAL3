using System;
using System.Collections.Generic;
using System.Text;

namespace CrossCom
{
    public class ExportedObject
    {
        public static ExportedObject CreateExportObjectFor<TInterface, TImplementation>()
            where TInterface : IUnknown
            where TImplementation : class, TInterface
        {
            return null;
        }
    }
}
