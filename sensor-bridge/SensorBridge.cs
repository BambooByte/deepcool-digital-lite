using System;
using System.Globalization;
using System.Runtime.InteropServices;
using System.Threading;
using LibreHardwareMonitor.Hardware;

internal static class SensorBridge
{
    [DllImport("kernel32.dll")]
    private static extern bool GetPhysicallyInstalledSystemMemory(out ulong totalMemoryInKilobytes);

    public static void Main(string[] args)
    {
        var computer = new Computer { IsCpuEnabled = true, IsMotherboardEnabled = true, IsGpuEnabled = true };
        computer.Open();
        try
        {
            string cpuName = "Unavailable";
            string gpuName = "Unavailable";
            foreach (var hardware in computer.Hardware)
            {
                if (hardware.HardwareType == HardwareType.Cpu)
                    cpuName = hardware.Name;
                if (hardware.HardwareType.ToString().StartsWith("Gpu", StringComparison.Ordinal))
                    gpuName = hardware.Name;
            }
            ulong memoryKb;
            string memory = GetPhysicallyInstalledSystemMemory(out memoryKb)
                ? (memoryKb / 1048576.0).ToString("0.#", CultureInfo.InvariantCulture) + " GB"
                : "Unavailable";
            Console.WriteLine("INFO\t{0}\t{1}\t{2}", cpuName.Replace('\t', ' '),
                gpuName.Replace('\t', ' '), memory);
            Console.Out.Flush();
            if (args.Length > 0 && args[0] == "--info")
                return;

            while (true)
            {
                float? temperature = null;
                float? power = null;
                float? usage = null;
                float? fan = null;
                float? cpuClock = null;
                float? gpuTemperature = null;
                float? gpuPower = null;
                float? gpuUsage = null;
                float? gpuClock = null;

                foreach (var hardware in computer.Hardware)
                {
                    Read(hardware, ref temperature, ref power, ref usage, ref fan, ref cpuClock,
                        ref gpuTemperature, ref gpuPower, ref gpuUsage, ref gpuClock);
                    foreach (var child in hardware.SubHardware)
                        Read(child, ref temperature, ref power, ref usage, ref fan, ref cpuClock,
                            ref gpuTemperature, ref gpuPower, ref gpuUsage, ref gpuClock);
                }

                Console.WriteLine("{0},{1},{2},{3},{4},{5},{6},{7},{8}",
                    Value(temperature), Value(power), Value(usage), Value(fan),
                    Value(gpuTemperature), Value(gpuPower), Value(gpuUsage), Value(gpuClock), Value(cpuClock));
                Console.Out.Flush();
                Thread.Sleep(1000);
            }
        }
        finally
        {
            computer.Close();
        }
    }

    private static string Value(float? value)
    {
        return value.HasValue ? value.Value.ToString("0.0", CultureInfo.InvariantCulture) : "missing";
    }

    private static void Read(IHardware hardware, ref float? temperature, ref float? power,
        ref float? usage, ref float? fan, ref float? cpuClock, ref float? gpuTemperature, ref float? gpuPower,
        ref float? gpuUsage, ref float? gpuClock)
    {
        hardware.Update();
        bool gpu = hardware.HardwareType.ToString().StartsWith("Gpu", StringComparison.Ordinal);
        foreach (var sensor in hardware.Sensors)
        {
            if (gpu)
            {
                if (sensor.SensorType == SensorType.Temperature && sensor.Name == "GPU Core")
                    gpuTemperature = sensor.Value;
                if (sensor.SensorType == SensorType.Power && (sensor.Name == "GPU Package" || sensor.Name == "GPU Power"))
                    gpuPower = sensor.Value;
                if (sensor.SensorType == SensorType.Load && sensor.Name == "GPU Core")
                    gpuUsage = sensor.Value;
                if (sensor.SensorType == SensorType.Clock && sensor.Name == "GPU Core")
                    gpuClock = sensor.Value;
                continue;
            }
            if (sensor.SensorType == SensorType.Temperature && sensor.Name == "CPU Package")
                temperature = sensor.Value;
            if (sensor.SensorType == SensorType.Power && sensor.Name == "CPU Package")
                power = sensor.Value;
            if (sensor.SensorType == SensorType.Load && sensor.Name == "CPU Total")
                usage = sensor.Value;
            if (sensor.SensorType == SensorType.Fan && sensor.Name == "Fan #1")
                fan = sensor.Value;
            if (hardware.HardwareType == HardwareType.Cpu && sensor.SensorType == SensorType.Clock && !cpuClock.HasValue)
                cpuClock = sensor.Value;
        }
    }
}
