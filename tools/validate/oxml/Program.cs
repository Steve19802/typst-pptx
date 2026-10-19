// Validates .pptx files with the Open XML SDK in PowerPoint 2021 mode.
// Exit code: 0 = no errors, 1 = validation errors, 2 = usage or open failure.
using DocumentFormat.OpenXml;
using DocumentFormat.OpenXml.Packaging;
using DocumentFormat.OpenXml.Validation;

if (args.Length == 0)
{
    Console.Error.WriteLine("usage: oxml-validate FILE.pptx...");
    return 2;
}

var validator = new OpenXmlValidator(FileFormatVersions.Office2021);
var exitCode = 0;
foreach (var path in args)
{
    try
    {
        using var doc = PresentationDocument.Open(path, false);
        var errors = validator.Validate(doc).ToList();
        foreach (var e in errors)
        {
            Console.WriteLine($"{path}: {e.Part?.Uri} {e.Path?.XPath}: [{e.Id}] {e.Description}");
        }
        Console.WriteLine($"{path}: {errors.Count} error(s)");
        if (errors.Count > 0)
        {
            exitCode = Math.Max(exitCode, 1);
        }
    }
    catch (Exception ex) when (ex is OpenXmlPackageException or IOException or InvalidDataException)
    {
        Console.WriteLine($"{path}: cannot open: {ex.Message}");
        exitCode = 2;
    }
}
return exitCode;
