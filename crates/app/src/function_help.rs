use crate::text;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Category {
    Logical,
    Text,
    DateTime,
    Lookup,
    Math,
    Statistical,
    Information,
}

impl Category {
    pub const ALL: [Self; 7] = [
        Self::Logical,
        Self::Text,
        Self::DateTime,
        Self::Lookup,
        Self::Math,
        Self::Statistical,
        Self::Information,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::Logical => text::logical(),
            Self::Text => text::text_functions(),
            Self::DateTime => text::date_time(),
            Self::Lookup => text::lookup(),
            Self::Math => text::math(),
            Self::Statistical => text::statistical(),
            Self::Information => text::information(),
        }
    }
}

pub struct Help {
    pub name: &'static str,
    pub category: Category,
    args_tr: &'static [&'static str],
    description_tr: &'static str,
    args_en: &'static [&'static str],
    description_en: &'static str,
}

impl Help {
    pub fn args(&self) -> &'static [&'static str] {
        if vellum_ui::english() {
            self.args_en
        } else {
            self.args_tr
        }
    }

    pub fn description(&self) -> &'static str {
        if vellum_ui::english() {
            self.description_en
        } else {
            self.description_tr
        }
    }
}

use Category::*;

macro_rules! help {
    (
        $name:literal,
        $category:ident,
        [$($arg:literal),*],
        $description:literal,
        [$($arg_en:literal),*],
        $description_en:literal
    ) => {
        Help {
            name: $name,
            category: $category,
            args_tr: &[$($arg),*],
            description_tr: $description,
            args_en: &[$($arg_en),*],
            description_en: $description_en,
        }
    };
}

pub const FUNCTIONS: &[Help] = &[
    help!(
        "ABS",
        Math,
        ["sayı"],
        "Bir sayının mutlak değerini verir.",
        ["number"],
        "Returns the absolute value of a number."
    ),
    help!(
        "AND",
        Logical,
        ["mantıksal1", "[mantıksal2]", "..."],
        "Tüm bağımsız değişkenler DOĞRU ise DOĞRU verir.",
        ["logical1", "[logical2]", "..."],
        "Returns TRUE if all of its arguments are TRUE."
    ),
    help!(
        "AVERAGE",
        Statistical,
        ["sayı1", "[sayı2]", "..."],
        "Bağımsız değişkenlerin aritmetik ortalamasını verir.",
        ["number1", "[number2]", "..."],
        "Returns the average (arithmetic mean) of its arguments."
    ),
    help!(
        "AVERAGEIF",
        Statistical,
        ["aralık", "ölçüt", "[ortalama_aralığı]"],
        "Ölçüte uyan hücrelerin ortalamasını verir.",
        ["range", "criteria", "[average_range]"],
        "Returns the average of the cells that meet a criterion."
    ),
    help!(
        "AVERAGEIFS",
        Statistical,
        ["ortalama_aralığı", "ölçüt_aralığı1", "ölçüt1", "..."],
        "Birden çok ölçüte uyan hücrelerin ortalamasını verir.",
        ["average_range", "criteria_range1", "criteria1", "..."],
        "Returns the average of the cells that meet multiple criteria."
    ),
    help!(
        "CEILING",
        Math,
        ["sayı", "anlam"],
        "Sayıyı en yakın anlam katına yukarı yuvarlar.",
        ["number", "significance"],
        "Rounds a number up to the nearest multiple of significance."
    ),
    help!(
        "CHAR",
        Text,
        ["sayı"],
        "Kod numarasıyla belirtilen karakteri verir.",
        ["number"],
        "Returns the character specified by the code number."
    ),
    help!(
        "CHOOSE",
        Lookup,
        ["dizin_sayısı", "değer1", "[değer2]", "..."],
        "Değerler listesinden dizine karşılık gelen değeri seçer.",
        ["index_num", "value1", "[value2]", "..."],
        "Chooses a value from a list of values based on an index number."
    ),
    help!(
        "CLEAN",
        Text,
        ["metin"],
        "Metinden yazdırılamayan karakterleri kaldırır.",
        ["text"],
        "Removes all nonprintable characters from text."
    ),
    help!(
        "CODE",
        Text,
        ["metin"],
        "Metindeki ilk karakterin kod numarasını verir.",
        ["text"],
        "Returns a numeric code for the first character in a text string."
    ),
    help!(
        "COLUMN",
        Lookup,
        ["[başvuru]"],
        "Başvurunun sütun numarasını verir.",
        ["[reference]"],
        "Returns the column number of a reference."
    ),
    help!(
        "COLUMNS",
        Lookup,
        ["dizi"],
        "Başvurudaki sütun sayısını verir.",
        ["array"],
        "Returns the number of columns in a reference."
    ),
    help!(
        "CONCAT",
        Text,
        ["metin1", "[metin2]", "..."],
        "Metin dizelerini ve aralıkları birleştirir.",
        ["text1", "[text2]", "..."],
        "Combines the text from multiple ranges and strings."
    ),
    help!(
        "CONCATENATE",
        Text,
        ["metin1", "[metin2]", "..."],
        "Birden çok metni tek metinde birleştirir.",
        ["text1", "[text2]", "..."],
        "Joins several text strings into one text string."
    ),
    help!(
        "COUNT",
        Statistical,
        ["değer1", "[değer2]", "..."],
        "Sayı içeren hücreleri sayar.",
        ["value1", "[value2]", "..."],
        "Counts the cells that contain numbers."
    ),
    help!(
        "COUNTA",
        Statistical,
        ["değer1", "[değer2]", "..."],
        "Boş olmayan hücreleri sayar.",
        ["value1", "[value2]", "..."],
        "Counts the cells that are not empty."
    ),
    help!(
        "COUNTBLANK",
        Statistical,
        ["aralık"],
        "Aralıktaki boş hücreleri sayar.",
        ["range"],
        "Counts the empty cells in a range."
    ),
    help!(
        "COUNTIF",
        Statistical,
        ["aralık", "ölçüt"],
        "Ölçüte uyan hücreleri sayar.",
        ["range", "criteria"],
        "Counts the cells that meet a criterion."
    ),
    help!(
        "COUNTIFS",
        Statistical,
        ["ölçüt_aralığı1", "ölçüt1", "..."],
        "Birden çok ölçüte uyan hücreleri sayar.",
        ["criteria_range1", "criteria1", "..."],
        "Counts the cells that meet multiple criteria."
    ),
    help!(
        "DATE",
        DateTime,
        ["yıl", "ay", "gün"],
        "Belirtilen tarihin seri numarasını verir.",
        ["year", "month", "day"],
        "Returns the serial number of a particular date."
    ),
    help!(
        "DATEDIF",
        DateTime,
        ["başlangıç_tarihi", "bitiş_tarihi", "birim"],
        "İki tarih arasındaki gün, ay veya yıl sayısını verir.",
        ["start_date", "end_date", "unit"],
        "Returns the number of days, months or years between two dates."
    ),
    help!(
        "DATEVALUE",
        DateTime,
        ["tarih_metni"],
        "Metin biçimindeki tarihi seri numarasına dönüştürür.",
        ["date_text"],
        "Converts a date in the form of text to a serial number."
    ),
    help!(
        "DAY",
        DateTime,
        ["seri_no"],
        "Ayın gününü (1-31) verir.",
        ["serial_number"],
        "Returns the day of the month (1-31)."
    ),
    help!(
        "DAYS",
        DateTime,
        ["bitiş_tarihi", "başlangıç_tarihi"],
        "İki tarih arasındaki gün sayısını verir.",
        ["end_date", "start_date"],
        "Returns the number of days between two dates."
    ),
    help!(
        "EDATE",
        DateTime,
        ["başlangıç_tarihi", "ay_sayısı"],
        "Başlangıçtan belirtilen ay sayısı önceki veya sonraki tarihi verir.",
        ["start_date", "months"],
        "Returns the date that is the indicated number of months before or after the start date."
    ),
    help!(
        "EOMONTH",
        DateTime,
        ["başlangıç_tarihi", "ay_sayısı"],
        "Belirtilen ay sayısı kadar önceki veya sonraki ayın son gününü verir.",
        ["start_date", "months"],
        "Returns the last day of the month before or after a specified number of months."
    ),
    help!(
        "EXACT",
        Text,
        ["metin1", "metin2"],
        "İki metnin tam olarak aynı olup olmadığını denetler.",
        ["text1", "text2"],
        "Checks whether two text values are exactly the same."
    ),
    help!(
        "EXP",
        Math,
        ["sayı"],
        "e sayısının belirtilen kuvvetini verir.",
        ["number"],
        "Returns e raised to the power of a given number."
    ),
    help!(
        "FALSE",
        Logical,
        [],
        "YANLIŞ mantıksal değerini verir.",
        [],
        "Returns the logical value FALSE."
    ),
    help!(
        "FIND",
        Text,
        ["bul_metni", "metin", "[başlangıç_sayısı]"],
        "Bir metni başka bir metin içinde bulur (büyük/küçük harfe duyarlı).",
        ["find_text", "within_text", "[start_num]"],
        "Finds one text value within another (case-sensitive)."
    ),
    help!(
        "FLOOR",
        Math,
        ["sayı", "anlam"],
        "Sayıyı en yakın anlam katına aşağı yuvarlar.",
        ["number", "significance"],
        "Rounds a number down to the nearest multiple of significance."
    ),
    help!(
        "HLOOKUP",
        Lookup,
        [
            "aranan_değer",
            "tablo_dizisi",
            "satır_indis_sayısı",
            "[aralık_bak]"
        ],
        "Tablonun ilk satırında arar ve belirtilen satırdaki değeri verir.",
        [
            "lookup_value",
            "table_array",
            "row_index_num",
            "[range_lookup]"
        ],
        "Looks in the top row of a table and returns the value in the indicated row."
    ),
    help!(
        "HOUR",
        DateTime,
        ["seri_no"],
        "Saati (0-23) verir.",
        ["serial_number"],
        "Returns the hour (0-23)."
    ),
    help!(
        "IF",
        Logical,
        [
            "mantıksal_sınama",
            "[eğer_doğruysa_değer]",
            "[eğer_yanlışsa_değer]"
        ],
        "Koşul DOĞRU ise bir değer, YANLIŞ ise başka bir değer verir.",
        ["logical_test", "[value_if_true]", "[value_if_false]"],
        "Returns one value if a condition is TRUE and another value if it is FALSE."
    ),
    help!(
        "IFERROR",
        Logical,
        ["değer", "eğer_hatalıysa_değer"],
        "İfade hata verirse belirtilen değeri, vermezse ifadenin kendisini verir.",
        ["value", "value_if_error"],
        "Returns a value you specify if a formula evaluates to an error; otherwise returns the result of the formula."
    ),
    help!(
        "IFNA",
        Logical,
        ["değer", "eğer_yoksa_değer"],
        "İfade #YOK verirse belirtilen değeri verir.",
        ["value", "value_if_na"],
        "Returns the value you specify if the expression resolves to #N/A."
    ),
    help!(
        "IFS",
        Logical,
        ["mantıksal_sınama1", "eğer_doğruysa_değer1", "..."],
        "İlk DOĞRU koşula karşılık gelen değeri verir.",
        ["logical_test1", "value_if_true1", "..."],
        "Returns the value that corresponds to the first TRUE condition."
    ),
    help!(
        "INDEX",
        Lookup,
        ["dizi", "satır_sayısı", "[sütun_sayısı]"],
        "Aralıkta belirtilen satır ve sütundaki değeri verir.",
        ["array", "row_num", "[column_num]"],
        "Returns the value at a given row and column of a range."
    ),
    help!(
        "INDIRECT",
        Lookup,
        ["başvuru_metni", "[a1]"],
        "Metinle belirtilen başvuruyu verir.",
        ["ref_text", "[a1]"],
        "Returns the reference specified by a text string."
    ),
    help!(
        "INT",
        Math,
        ["sayı"],
        "Sayıyı en yakın tamsayıya aşağı yuvarlar.",
        ["number"],
        "Rounds a number down to the nearest integer."
    ),
    help!(
        "ISBLANK",
        Information,
        ["değer"],
        "Değer boşsa DOĞRU verir.",
        ["value"],
        "Returns TRUE if the value is blank."
    ),
    help!(
        "ISERR",
        Information,
        ["değer"],
        "Değer #YOK dışında bir hataysa DOĞRU verir.",
        ["value"],
        "Returns TRUE if the value is any error other than #N/A."
    ),
    help!(
        "ISERROR",
        Information,
        ["değer"],
        "Değer herhangi bir hataysa DOĞRU verir.",
        ["value"],
        "Returns TRUE if the value is any error."
    ),
    help!(
        "ISLOGICAL",
        Information,
        ["değer"],
        "Değer mantıksal bir değerse DOĞRU verir.",
        ["value"],
        "Returns TRUE if the value is a logical value."
    ),
    help!(
        "ISNA",
        Information,
        ["değer"],
        "Değer #YOK hatasıysa DOĞRU verir.",
        ["value"],
        "Returns TRUE if the value is the #N/A error."
    ),
    help!(
        "ISNUMBER",
        Information,
        ["değer"],
        "Değer sayıysa DOĞRU verir.",
        ["value"],
        "Returns TRUE if the value is a number."
    ),
    help!(
        "ISTEXT",
        Information,
        ["değer"],
        "Değer metinse DOĞRU verir.",
        ["value"],
        "Returns TRUE if the value is text."
    ),
    help!(
        "LARGE",
        Statistical,
        ["dizi", "k"],
        "Veri kümesindeki k. en büyük değeri verir.",
        ["array", "k"],
        "Returns the k-th largest value in a data set."
    ),
    help!(
        "LEFT",
        Text,
        ["metin", "[sayı_karakterler]"],
        "Metnin başından belirtilen sayıda karakter verir.",
        ["text", "[num_chars]"],
        "Returns the specified number of characters from the start of a text string."
    ),
    help!(
        "LEN",
        Text,
        ["metin"],
        "Metindeki karakter sayısını verir.",
        ["text"],
        "Returns the number of characters in a text string."
    ),
    help!(
        "LN",
        Math,
        ["sayı"],
        "Sayının doğal logaritmasını verir.",
        ["number"],
        "Returns the natural logarithm of a number."
    ),
    help!(
        "LOG",
        Math,
        ["sayı", "[taban]"],
        "Sayının belirtilen tabandaki logaritmasını verir.",
        ["number", "[base]"],
        "Returns the logarithm of a number to the specified base."
    ),
    help!(
        "LOG10",
        Math,
        ["sayı"],
        "Sayının 10 tabanındaki logaritmasını verir.",
        ["number"],
        "Returns the base-10 logarithm of a number."
    ),
    help!(
        "LOWER",
        Text,
        ["metin"],
        "Metindeki tüm harfleri küçük harfe çevirir.",
        ["text"],
        "Converts all letters in a text string to lowercase."
    ),
    help!(
        "MATCH",
        Lookup,
        ["aranan_değer", "aranan_dizi", "[eşleştir_tür]"],
        "Aranan değerin aralıktaki göreli konumunu verir.",
        ["lookup_value", "lookup_array", "[match_type]"],
        "Returns the relative position of an item in a range."
    ),
    help!(
        "MAX",
        Statistical,
        ["sayı1", "[sayı2]", "..."],
        "En büyük değeri verir.",
        ["number1", "[number2]", "..."],
        "Returns the largest value."
    ),
    help!(
        "MAXIFS",
        Statistical,
        ["en_büyük_aralığı", "ölçüt_aralığı1", "ölçüt1", "..."],
        "Ölçütlere uyan hücrelerin en büyük değerini verir.",
        ["max_range", "criteria_range1", "criteria1", "..."],
        "Returns the maximum value among cells that meet the criteria."
    ),
    help!(
        "MEDIAN",
        Statistical,
        ["sayı1", "[sayı2]", "..."],
        "Sayıların ortancasını verir.",
        ["number1", "[number2]", "..."],
        "Returns the median of the given numbers."
    ),
    help!(
        "MID",
        Text,
        ["metin", "başlangıç_sayısı", "sayı_karakterler"],
        "Metnin belirtilen konumundan başlayarak karakter verir.",
        ["text", "start_num", "num_chars"],
        "Returns characters from the middle of a text string, starting at the position you specify."
    ),
    help!(
        "MIN",
        Statistical,
        ["sayı1", "[sayı2]", "..."],
        "En küçük değeri verir.",
        ["number1", "[number2]", "..."],
        "Returns the smallest value."
    ),
    help!(
        "MINIFS",
        Statistical,
        ["en_küçük_aralığı", "ölçüt_aralığı1", "ölçüt1", "..."],
        "Ölçütlere uyan hücrelerin en küçük değerini verir.",
        ["min_range", "criteria_range1", "criteria1", "..."],
        "Returns the minimum value among cells that meet the criteria."
    ),
    help!(
        "MINUTE",
        DateTime,
        ["seri_no"],
        "Dakikayı (0-59) verir.",
        ["serial_number"],
        "Returns the minute (0-59)."
    ),
    help!(
        "MOD",
        Math,
        ["sayı", "bölen"],
        "Bölme işleminden kalanı verir.",
        ["number", "divisor"],
        "Returns the remainder after division."
    ),
    help!(
        "MONTH",
        DateTime,
        ["seri_no"],
        "Ayı (1-12) verir.",
        ["serial_number"],
        "Returns the month (1-12)."
    ),
    help!(
        "MROUND",
        Math,
        ["sayı", "katsayı"],
        "Sayıyı istenen katsayıya yuvarlar.",
        ["number", "multiple"],
        "Returns a number rounded to the desired multiple."
    ),
    help!(
        "NA",
        Information,
        [],
        "#YOK hata değerini verir.",
        [],
        "Returns the error value #N/A."
    ),
    help!(
        "NETWORKDAYS",
        DateTime,
        ["başlangıç_tarihi", "bitiş_tarihi", "[tatiller]"],
        "İki tarih arasındaki iş günü sayısını verir.",
        ["start_date", "end_date", "[holidays]"],
        "Returns the number of whole workdays between two dates."
    ),
    help!(
        "NOT",
        Logical,
        ["mantıksal"],
        "Mantıksal değeri tersine çevirir.",
        ["logical"],
        "Reverses the logic of its argument."
    ),
    help!(
        "NOW",
        DateTime,
        [],
        "Geçerli tarih ve saati verir.",
        [],
        "Returns the current date and time."
    ),
    help!(
        "OFFSET",
        Lookup,
        [
            "başvuru",
            "satırlar",
            "sütunlar",
            "[yükseklik]",
            "[genişlik]"
        ],
        "Başvurudan belirtilen satır ve sütun uzaklığındaki aralığı verir.",
        ["reference", "rows", "cols", "[height]", "[width]"],
        "Returns a reference offset from a given reference by rows and columns."
    ),
    help!(
        "OR",
        Logical,
        ["mantıksal1", "[mantıksal2]", "..."],
        "Bağımsız değişkenlerden biri DOĞRU ise DOĞRU verir.",
        ["logical1", "[logical2]", "..."],
        "Returns TRUE if any argument is TRUE."
    ),
    help!(
        "PI",
        Math,
        [],
        "Pi sayısını verir.",
        [],
        "Returns the value of pi."
    ),
    help!(
        "POWER",
        Math,
        ["sayı", "üs"],
        "Sayının belirtilen kuvvetini verir.",
        ["number", "power"],
        "Returns the result of a number raised to a power."
    ),
    help!(
        "PRODUCT",
        Math,
        ["sayı1", "[sayı2]", "..."],
        "Bağımsız değişkenlerin çarpımını verir.",
        ["number1", "[number2]", "..."],
        "Multiplies its arguments."
    ),
    help!(
        "PROPER",
        Text,
        ["metin"],
        "Her sözcüğün ilk harfini büyük yapar.",
        ["text"],
        "Capitalizes the first letter in each word of a text value."
    ),
    help!(
        "RAND",
        Math,
        [],
        "0 ile 1 arasında rastgele bir sayı verir.",
        [],
        "Returns a random number between 0 and 1."
    ),
    help!(
        "RANDBETWEEN",
        Math,
        ["alt", "üst"],
        "İki sayı arasında rastgele bir tamsayı verir.",
        ["bottom", "top"],
        "Returns a random integer between the numbers you specify."
    ),
    help!(
        "RANK",
        Statistical,
        ["sayı", "başv", "[sıra]"],
        "Sayının listedeki sırasını verir.",
        ["number", "ref", "[order]"],
        "Returns the rank of a number in a list of numbers."
    ),
    help!(
        "REPLACE",
        Text,
        [
            "eski_metin",
            "başlangıç_sayısı",
            "sayı_karakterler",
            "yeni_metin"
        ],
        "Metnin bir bölümünü başka bir metinle değiştirir.",
        ["old_text", "start_num", "num_chars", "new_text"],
        "Replaces part of a text string with a different text string."
    ),
    help!(
        "REPT",
        Text,
        ["metin", "sayı_kere"],
        "Metni belirtilen sayıda yineler.",
        ["text", "number_times"],
        "Repeats text a given number of times."
    ),
    help!(
        "RIGHT",
        Text,
        ["metin", "[sayı_karakterler]"],
        "Metnin sonundan belirtilen sayıda karakter verir.",
        ["text", "[num_chars]"],
        "Returns the specified number of characters from the end of a text string."
    ),
    help!(
        "ROUND",
        Math,
        ["sayı", "sayı_rakamlar"],
        "Sayıyı belirtilen basamak sayısına yuvarlar.",
        ["number", "num_digits"],
        "Rounds a number to a specified number of digits."
    ),
    help!(
        "ROUNDDOWN",
        Math,
        ["sayı", "sayı_rakamlar"],
        "Sayıyı sıfıra doğru aşağı yuvarlar.",
        ["number", "num_digits"],
        "Rounds a number down, toward zero."
    ),
    help!(
        "ROUNDUP",
        Math,
        ["sayı", "sayı_rakamlar"],
        "Sayıyı sıfırdan uzağa yukarı yuvarlar.",
        ["number", "num_digits"],
        "Rounds a number up, away from zero."
    ),
    help!(
        "ROW",
        Lookup,
        ["[başvuru]"],
        "Başvurunun satır numarasını verir.",
        ["[reference]"],
        "Returns the row number of a reference."
    ),
    help!(
        "ROWS",
        Lookup,
        ["dizi"],
        "Başvurudaki satır sayısını verir.",
        ["array"],
        "Returns the number of rows in a reference."
    ),
    help!(
        "SEARCH",
        Text,
        ["bul_metni", "metin", "[başlangıç_sayısı]"],
        "Bir metni başka bir metin içinde bulur (büyük/küçük harfe duyarsız).",
        ["find_text", "within_text", "[start_num]"],
        "Finds one text value within another (not case-sensitive)."
    ),
    help!(
        "SECOND",
        DateTime,
        ["seri_no"],
        "Saniyeyi (0-59) verir.",
        ["serial_number"],
        "Returns the second (0-59)."
    ),
    help!(
        "SIGN",
        Math,
        ["sayı"],
        "Sayının işaretini (1, 0 veya -1) verir.",
        ["number"],
        "Returns the sign of a number (1, 0 or -1)."
    ),
    help!(
        "SMALL",
        Statistical,
        ["dizi", "k"],
        "Veri kümesindeki k. en küçük değeri verir.",
        ["array", "k"],
        "Returns the k-th smallest value in a data set."
    ),
    help!(
        "SQRT",
        Math,
        ["sayı"],
        "Sayının karekökünü verir.",
        ["number"],
        "Returns a positive square root."
    ),
    help!(
        "STDEV",
        Statistical,
        ["sayı1", "[sayı2]", "..."],
        "Örneğe dayalı standart sapmayı verir.",
        ["number1", "[number2]", "..."],
        "Estimates standard deviation based on a sample."
    ),
    help!(
        "SUBSTITUTE",
        Text,
        ["metin", "eski_metin", "yeni_metin", "[yineleme_sayısı]"],
        "Metindeki eski metni yeni metinle değiştirir.",
        ["text", "old_text", "new_text", "[instance_num]"],
        "Substitutes new text for old text in a text string."
    ),
    help!(
        "SUBTOTAL",
        Math,
        ["işlev_sayısı", "başv1", "..."],
        "Gizli satırları yok sayarak alt toplam verir (9 = TOPLA, 1 = ORTALAMA…).",
        ["function_num", "ref1", "..."],
        "Returns a subtotal that ignores hidden rows (9 = SUM, 1 = AVERAGE…)."
    ),
    help!(
        "SUM",
        Math,
        ["sayı1", "[sayı2]", "..."],
        "Sayıları toplar.",
        ["number1", "[number2]", "..."],
        "Adds its arguments."
    ),
    help!(
        "SUMIF",
        Math,
        ["aralık", "ölçüt", "[toplam_aralığı]"],
        "Ölçüte uyan hücreleri toplar.",
        ["range", "criteria", "[sum_range]"],
        "Adds the cells that meet a criterion."
    ),
    help!(
        "SUMIFS",
        Math,
        ["toplam_aralığı", "ölçüt_aralığı1", "ölçüt1", "..."],
        "Birden çok ölçüte uyan hücreleri toplar.",
        ["sum_range", "criteria_range1", "criteria1", "..."],
        "Adds the cells that meet multiple criteria."
    ),
    help!(
        "SUMPRODUCT",
        Math,
        ["dizi1", "[dizi2]", "..."],
        "Karşılık gelen aralık değerlerinin çarpımlarının toplamını verir.",
        ["array1", "[array2]", "..."],
        "Returns the sum of the products of corresponding range components."
    ),
    help!(
        "SWITCH",
        Logical,
        ["ifade", "değer1", "sonuç1", "...", "[varsayılan]"],
        "İfadeyi değerlerle karşılaştırır ve ilk eşleşmenin sonucunu verir.",
        ["expression", "value1", "result1", "...", "[default]"],
        "Compares an expression against a list of values and returns the result of the first match."
    ),
    help!(
        "TEXT",
        Text,
        ["değer", "biçim_metni"],
        "Değeri belirtilen sayı biçimiyle metne dönüştürür.",
        ["value", "format_text"],
        "Formats a number and converts it to text."
    ),
    help!(
        "TEXTJOIN",
        Text,
        ["sınırlayıcı", "boşluğu_yoksay", "metin1", "..."],
        "Metinleri sınırlayıcıyla birleştirir.",
        ["delimiter", "ignore_empty", "text1", "..."],
        "Combines text values with a delimiter."
    ),
    help!(
        "TIME",
        DateTime,
        ["saat", "dakika", "saniye"],
        "Belirtilen saatin seri numarasını verir.",
        ["hour", "minute", "second"],
        "Returns the serial number of a particular time."
    ),
    help!(
        "TODAY",
        DateTime,
        [],
        "Bugünün tarihini verir.",
        [],
        "Returns today's date."
    ),
    help!(
        "TRIM",
        Text,
        ["metin"],
        "Sözcükler arasındaki tek boşluk dışındaki boşlukları kaldırır.",
        ["text"],
        "Removes spaces from text except for single spaces between words."
    ),
    help!(
        "TRUE",
        Logical,
        [],
        "DOĞRU mantıksal değerini verir.",
        [],
        "Returns the logical value TRUE."
    ),
    help!(
        "TRUNC",
        Math,
        ["sayı", "[sayı_rakamlar]"],
        "Sayının ondalık kısmını keser.",
        ["number", "[num_digits]"],
        "Truncates a number by removing its fractional part."
    ),
    help!(
        "UPPER",
        Text,
        ["metin"],
        "Metindeki tüm harfleri büyük harfe çevirir.",
        ["text"],
        "Converts text to uppercase."
    ),
    help!(
        "VALUE",
        Text,
        ["metin"],
        "Sayıyı gösteren metni sayıya dönüştürür.",
        ["text"],
        "Converts a text string that represents a number to a number."
    ),
    help!(
        "VLOOKUP",
        Lookup,
        [
            "aranan_değer",
            "tablo_dizisi",
            "sütun_indis_sayısı",
            "[aralık_bak]"
        ],
        "Tablonun ilk sütununda arar ve belirtilen sütundaki değeri verir.",
        [
            "lookup_value",
            "table_array",
            "col_index_num",
            "[range_lookup]"
        ],
        "Looks in the first column of a table and returns the value in the indicated column."
    ),
    help!(
        "WEEKDAY",
        DateTime,
        ["seri_no", "[döndür_tür]"],
        "Haftanın gününü sayı olarak verir.",
        ["serial_number", "[return_type]"],
        "Returns the day of the week as a number."
    ),
    help!(
        "WEEKNUM",
        DateTime,
        ["seri_no", "[döndür_tür]"],
        "Yılın kaçıncı haftası olduğunu verir.",
        ["serial_number", "[return_type]"],
        "Returns the week number of the year."
    ),
    help!(
        "WORKDAY",
        DateTime,
        ["başlangıç_tarihi", "gün_sayısı", "[tatiller]"],
        "Belirtilen iş günü sayısı kadar önceki veya sonraki tarihi verir.",
        ["start_date", "days", "[holidays]"],
        "Returns the date before or after a specified number of workdays."
    ),
    help!(
        "XLOOKUP",
        Lookup,
        [
            "aranan",
            "arama_dizisi",
            "döndürülen_dizi",
            "[bulunamazsa]",
            "[eşleştirme_modu]",
            "[arama_modu]"
        ],
        "Aralıkta arar ve karşılık gelen değeri verir.",
        [
            "lookup_value",
            "lookup_array",
            "return_array",
            "[if_not_found]",
            "[match_mode]",
            "[search_mode]"
        ],
        "Searches a range and returns the corresponding item."
    ),
    help!(
        "XOR",
        Logical,
        ["mantıksal1", "[mantıksal2]", "..."],
        "Bağımsız değişkenlerin dışlayıcı VEYA sonucunu verir.",
        ["logical1", "[logical2]", "..."],
        "Returns the logical exclusive OR of all arguments."
    ),
    help!(
        "YEAR",
        DateTime,
        ["seri_no"],
        "Yılı verir.",
        ["serial_number"],
        "Returns the year."
    ),
];

pub fn find(canonical: &str) -> Option<&'static Help> {
    FUNCTIONS.iter().find(|help| help.name == canonical)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_engine_function_has_help() {
        let names = vellum_grid_core::formula::function_names();
        let missing: Vec<_> = names.iter().filter(|name| find(name).is_none()).collect();
        assert!(missing.is_empty(), "{missing:?}");
        assert_eq!(FUNCTIONS.len(), names.len());
    }
}
