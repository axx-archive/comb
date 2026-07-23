import type { Metadata } from "next";
import { Archivo_Black, IBM_Plex_Mono, Instrument_Sans } from "next/font/google";
import { headers } from "next/headers";
import "./globals.css";

const archivo = Archivo_Black({
  variable: "--font-archivo",
  subsets: ["latin"],
  weight: "400",
});

const instrument = Instrument_Sans({
  variable: "--font-instrument",
  subsets: ["latin"],
});

const plex = IBM_Plex_Mono({
  variable: "--font-plex",
  subsets: ["latin"],
  weight: ["400", "500", "600"],
});

const baseMetadata: Metadata = {
  title: {
    default: "Comb — DLC for Buzz",
    template: "%s — Comb",
  },
  description:
    "Evidence-backed organizational memory for Buzz. Comb turns signed channel conversations into ratified project knowledge with receipts.",
  applicationName: "Comb",
  keywords: [
    "Buzz",
    "organizational memory",
    "agent collaboration",
    "open source",
    "project intelligence",
  ],
};

export async function generateMetadata(): Promise<Metadata> {
  const requestHeaders = await headers();
  const forwardedHost = requestHeaders.get("x-forwarded-host")?.split(",")[0]?.trim();
  const requestHost = forwardedHost ?? requestHeaders.get("host") ?? "localhost";
  const safeHost = /^[a-z0-9.-]+(?::\d+)?$/i.test(requestHost) ? requestHost : "localhost";
  const forwardedProtocol = requestHeaders.get("x-forwarded-proto")?.split(",")[0]?.trim();
  const protocol = forwardedProtocol === "http" || safeHost.startsWith("localhost") ? "http" : "https";
  const siteUrl = new URL(`${protocol}://${safeHost}`);
  const socialImage = new URL("/og.png", siteUrl);

  return {
    ...baseMetadata,
    metadataBase: siteUrl,
    openGraph: {
      title: "Your workspace is buzzing. Keep what it learns.",
      description: "Independent open-source organizational memory for Buzz. Built by That’s Cool.",
      type: "website",
      siteName: "Comb",
      url: siteUrl,
      images: [
        {
          url: socialImage,
          width: 1731,
          height: 909,
          alt: "Comb — Your workspace is buzzing. Keep what it learns.",
        },
      ],
    },
    twitter: {
      card: "summary_large_image",
      title: "Your workspace is buzzing. Keep what it learns.",
      description: "Independent open-source organizational memory for Buzz. Built by That’s Cool.",
      images: [socialImage],
    },
  };
}

export default function RootLayout({
  children,
}: Readonly<{
  children: React.ReactNode;
}>) {
  return (
    <html lang="en">
      <body className={`${archivo.variable} ${instrument.variable} ${plex.variable}`}>
        {children}
      </body>
    </html>
  );
}
