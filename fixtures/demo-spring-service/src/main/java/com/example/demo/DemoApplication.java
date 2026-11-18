package com.example.demo;

public class DemoApplication {
    public static void main(String[] args) throws Exception {
        System.out.println("  .   ____          _            __ _ _");
        System.out.println(" /\\\\ / ___'_ __ _ _(_)_ __  __ _ \\ \\ \\ \\");
        System.out.println("( ( )\\___ | '_ | '_| | '_ \\/ _` | \\ \\ \\ \\");
        System.out.println(" \\\\/  ___)| |_)| | | | | || (_| |  ) ) ) )");
        System.out.println("  '  |____| .__|_| |_|_| |_\\__, | / / / /");
        System.out.println(" =========|_|==============|___/=/_/_/_/");
        System.out.println(" :: Spring Boot ::                (v3.2.0)");
        System.out.println();

        String activeProfiles = System.getProperty("spring.profiles.active");
        if (activeProfiles == null) {
            activeProfiles = System.getProperty("spring-boot.run.profiles");
        }
        for (String arg : args) {
            if (arg.startsWith("--spring.profiles.active=")) {
                activeProfiles = arg.substring("--spring.profiles.active=".length());
            }
        }
        if (activeProfiles == null || activeProfiles.trim().isEmpty()) {
            activeProfiles = "default";
        }

        System.out.println("2026-11-17T16:20:00.123+08:00  INFO 12345 --- [main] c.e.demo.DemoApplication : Starting DemoApplication with PID 12345");
        System.out.println("2026-11-17T16:20:00.124+08:00  INFO 12345 --- [main] c.e.demo.DemoApplication : The following 1 profile is active: \"" + activeProfiles + "\"");
        System.out.println("2026-11-17T16:20:01.500+08:00  INFO 12345 --- [main] o.s.b.w.e.tomcat.TomcatWebServer  : Tomcat started on port 8080 (http)");
        System.out.println("2026-11-17T16:20:01.510+08:00  INFO 12345 --- [main] c.e.demo.DemoApplication : Started DemoApplication in 1.85 seconds (process running)");

        int count = 0;
        while (count < 100) {
            Thread.sleep(1000);
            count++;
            System.out.println("2026-11-17T16:20:" + String.format("%02d", count) + ".000+08:00  INFO 12345 --- [scheduling-1] c.e.demo.DemoApplication : Heartbeat tick #" + count + " [profile=" + activeProfiles + "]");
        }
    }
}
